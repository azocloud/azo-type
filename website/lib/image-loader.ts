import type { ImageLoaderProps } from 'next/image';
import { toOptimizedImageUrl } from './image-optimization.mjs';

export default function staticImageLoader({ src, width }: ImageLoaderProps): string {
  return toOptimizedImageUrl(
    src,
    width,
    process.env.NODE_ENV === 'production',
  );
}
