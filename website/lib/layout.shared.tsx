import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { appName, gitConfig } from './shared';

function AzoTypeMark() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 553 392"
      className="size-5 shrink-0"
      aria-hidden="true"
    >
      <polygon
        points="0,392 252,0 339.43,136 259.07,136 94.5,392"
        fill="currentColor"
      />
      <polygon
        points="270.54,157 553,157 514.43,217 444.5,217 332,392 252,392 364.5,217 231.97,217"
        fill="currentColor"
      />
      <polygon
        points="493,332 553,332 514.43,392 454.43,392"
        fill="currentColor"
      />
    </svg>
  );
}

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: (
        <span className="inline-flex items-center gap-2 font-semibold tracking-tight">
          <AzoTypeMark />
          <span>{appName}</span>
        </span>
      ),
      url: '/',
    },
    links: [
      {
        text: 'Home',
        url: '/',
        active: 'url',
      },
      {
        text: 'Download',
        url: '/download',
        active: 'nested-url',
      },
      {
        text: 'Documentation',
        url: '/docs',
        active: 'nested-url',
      },
    ],
    githubUrl: `https://github.com/${gitConfig.user}/${gitConfig.repo}`,
  };
}
