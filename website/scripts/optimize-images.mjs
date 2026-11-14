import fs from 'node:fs/promises';
import path from 'node:path';
import sharp from 'sharp';
import {
  ALL_IMAGE_WIDTHS,
  IMAGE_WEBP_QUALITY,
  isOptimizableRasterPath,
  toOptimizedImageUrl,
} from '../lib/image-optimization.mjs';

const OUT_DIR = path.resolve(import.meta.dirname, '..', 'out');
const SKIP_DIRS = new Set(['chunks', 'og']);

async function collectFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      files.push(...(await collectFiles(fullPath)));
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  }
  return files;
}

async function readStaticTagReferences(outFiles) {
  const referencedBasenames = new Set();
  const textFiles = outFiles.filter((file) =>
    file.endsWith('.html') || file.endsWith('.css'),
  );

  for (const file of textFiles) {
    const content = await fs.readFile(file, 'utf8');
    // Match attribute values and CSS url(...) outside inline Next Flight script strings
    const stripped = file.endsWith('.html')
      ? content.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '')
      : content;

    const matches = stripped.matchAll(
      /(?:src|href|content)=["']([^"']+)["']|url\(["']?([^)"']+)["']?\)/gi,
    );
    for (const match of matches) {
      const target = (match[1] ?? match[2] ?? '').split(/[?#]/, 1)[0];
      if (target) {
        referencedBasenames.add(path.basename(target));
      }
    }
  }
  return referencedBasenames;
}

function formatKb(bytes) {
  return `${(bytes / 1024).toFixed(1)} KiB`;
}

async function optimizeExportImages() {
  const allOutFiles = await collectFiles(OUT_DIR);
  const directRefs = await readStaticTagReferences(allOutFiles);

  const rasterFiles = allOutFiles.filter((file) => {
    const relUrl = `/${path.relative(OUT_DIR, file).split(path.sep).join('/')}`;
    return isOptimizableRasterPath(relUrl);
  });

  if (rasterFiles.length === 0) {
    console.log('No raster images found to optimize in website/out.');
    return;
  }

  let totalRawBytes = 0;
  let totalLargestWebpBytes = 0;
  let variantsWritten = 0;

  for (const sourceFile of rasterFiles) {
    const inputBuffer = await fs.readFile(sourceFile);
    const stat = await fs.stat(sourceFile);
    totalRawBytes += stat.size;

    const relUrl = `/${path.relative(OUT_DIR, sourceFile).split(path.sep).join('/')}`;
    let smallest = Infinity;
    let largest = 0;

    for (const width of ALL_IMAGE_WIDTHS) {
      const targetRelUrl = toOptimizedImageUrl(relUrl, width, true);
      const targetFile = path.join(OUT_DIR, targetRelUrl.slice(1));

      const info = await sharp(inputBuffer)
        .resize({ width, withoutEnlargement: true })
        .webp({ quality: IMAGE_WEBP_QUALITY, effort: 5 })
        .toFile(targetFile);

      smallest = Math.min(smallest, info.size);
      largest = Math.max(largest, info.size);
      variantsWritten += 1;
    }

    totalLargestWebpBytes += largest;

    const basename = path.basename(sourceFile);
    const inNextMedia = sourceFile.includes(
      `${path.sep}_next${path.sep}static${path.sep}media${path.sep}`,
    );
    const pruned = inNextMedia && !directRefs.has(basename);
    if (pruned) {
      await fs.unlink(sourceFile);
    }

    console.log(
      `[optimize-images] ${path.relative(OUT_DIR, sourceFile)}: ${formatKb(stat.size)} -> ${formatKb(smallest)}..${formatKb(largest)} WebP (${ALL_IMAGE_WIDTHS.length} widths${pruned ? ', raw asset pruned' : ''})`,
    );
  }

  console.log(
    `[optimize-images] Generated ${variantsWritten} WebP variants for ${rasterFiles.length} asset(s); max desktop variant sum ${formatKb(totalLargestWebpBytes)} vs raw ${formatKb(totalRawBytes)}.`,
  );
}

await optimizeExportImages();
