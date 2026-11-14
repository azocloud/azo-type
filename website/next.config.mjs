import { createMDX } from 'fumadocs-mdx/next';
import {
  IMAGE_DEVICE_SIZES,
  IMAGE_SIZES,
} from './lib/image-optimization.mjs';

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  output: 'export',
  images: {
    loader: 'custom',
    loaderFile: './lib/image-loader.ts',
    deviceSizes: IMAGE_DEVICE_SIZES,
    imageSizes: IMAGE_SIZES,
  },
  // Active configuration for GitHub Pages subpath (https://azocloud.github.io/azo-type)
  basePath: '/azo-type',
  // Custom domain: https://azotype.azocloud.de (no basePath needed)
  reactStrictMode: true,
};

export default withMDX(config);
