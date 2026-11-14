export const IMAGE_SIZES = [256, 384];
export const IMAGE_DEVICE_SIZES = [640, 828, 1200, 1920, 2560];
export const ALL_IMAGE_WIDTHS = [...IMAGE_SIZES, ...IMAGE_DEVICE_SIZES];
export const IMAGE_WEBP_QUALITY = 86;

const RASTER_EXT_RE = /\.(png|jpe?g|webp|avif)$/i;
const OPTIMIZED_SUFFIX_RE = /-w\d+\.webp$/i;
const EXTERNAL_URL_RE = /^(?:[a-z]+:)?\/\//i;

export function isOptimizableRasterPath(pathname) {
  if (!pathname) return false;
  if (EXTERNAL_URL_RE.test(pathname) || pathname.startsWith('data:')) {
    return false;
  }
  const cleanPath = pathname.split(/[?#]/, 1)[0];
  if (!RASTER_EXT_RE.test(cleanPath) || OPTIMIZED_SUFFIX_RE.test(cleanPath)) {
    return false;
  }
  // Leave dynamic route/OpenGraph assets untouched.
  if (cleanPath.includes('/og/')) {
    return false;
  }
  return true;
}

export function toOptimizedImageUrl(src, width, isProduction = true) {
  if (!isOptimizableRasterPath(src)) {
    return src;
  }

  const [pathname, query = ''] = src.split('?', 2);
  if (!isProduction) {
    const separator = query ? '&' : '?';
    return `${src}${separator}w=${width}`;
  }

  const optimizedPathname = pathname.replace(RASTER_EXT_RE, `-w${width}.webp`);
  return query ? `${optimizedPathname}?${query}` : optimizedPathname;
}
