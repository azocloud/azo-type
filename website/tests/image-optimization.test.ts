import test from 'node:test';
import assert from 'node:assert/strict';

import {
  ALL_IMAGE_WIDTHS,
  isOptimizableRasterPath,
  toOptimizedImageUrl,
} from '../lib/image-optimization.mjs';

test('isOptimizableRasterPath accepts local raster images and skips svgs, og routes, external urls, and generated variants', () => {
  assert.equal(
    isOptimizableRasterPath('/azo-type/_next/static/media/screenshot.abc123.png'),
    true,
  );
  assert.equal(isOptimizableRasterPath('/images/banner.jpg'), true);
  assert.equal(isOptimizableRasterPath('/azo-type/_next/static/media/icon.svg'), false);
  assert.equal(isOptimizableRasterPath('/azo-type/og/docs/features/image.png'), false);
  assert.equal(
    isOptimizableRasterPath('/azo-type/_next/static/media/screenshot.abc123-w1200.webp'),
    false,
  );
  assert.equal(isOptimizableRasterPath('https://example.com/photo.png'), false);
  assert.equal(isOptimizableRasterPath('data:image/png;base64,AAAA'), false);
});

test('toOptimizedImageUrl rewrites local raster URLs for production static exports and dev mode', () => {
  assert.equal(
    toOptimizedImageUrl('/azo-type/_next/static/media/screenshot.41gm.png', 1200, true),
    '/azo-type/_next/static/media/screenshot.41gm-w1200.webp',
  );
  assert.equal(
    toOptimizedImageUrl('/images/demo.jpeg?v=2', 640, true),
    '/images/demo-w640.webp?v=2',
  );
  assert.equal(
    toOptimizedImageUrl('/azo-type/_next/static/media/screenshot.41gm.png', 1200, false),
    '/azo-type/_next/static/media/screenshot.41gm.png?w=1200',
  );
  assert.equal(
    toOptimizedImageUrl('/azo-type/_next/static/media/icon.svg', 640, true),
    '/azo-type/_next/static/media/icon.svg',
  );
  assert.ok(ALL_IMAGE_WIDTHS.includes(1200));
});
