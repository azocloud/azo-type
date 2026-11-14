import { createMDX } from 'fumadocs-mdx/next';

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  output: 'export',
  images: { unoptimized: true },
  // Active configuration for GitHub Pages subpath (https://azocloud.github.io/azo-type)
  basePath: '/azo-type',
  // Custom domain: https://azotype.azocloud.de (no basePath needed)
  reactStrictMode: true,
};

export default withMDX(config);
