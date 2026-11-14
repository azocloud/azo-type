'use client';

import { useId, useState } from 'react';
import Image from 'next/image';
import { MoveHorizontal } from 'lucide-react';
import darkScreenshot from '@/assets/screenshot.png';
import lightScreenshot from '@/assets/screenshot-light.png';

const SCREENSHOT_SIZES = '(max-width: 1152px) calc(100vw - 3rem), 1104px';

export function ThemeCompareSlider() {
  const [position, setPosition] = useState(50);
  const sliderId = useId();

  return (
    <div className="relative overflow-hidden select-none">
      {/* Base Layer: Light theme screenshot */}
      <Image
        src={lightScreenshot}
        alt="Azo Type desktop application window comparing Dark theme on the left and Light theme on the right, showing the file tree, document outline, code editor, and live document preview."
        sizes={SCREENSHOT_SIZES}
        className="pointer-events-none block h-auto w-full select-none"
        draggable={false}
        priority
      />

      {/* Overlay Layer: Dark theme screenshot clipped at slider position */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0"
        style={{ clipPath: `inset(0 ${100 - position}% 0 0)` }}
      >
        <Image
          src={darkScreenshot}
          alt=""
          sizes={SCREENSHOT_SIZES}
          className="block h-full w-full select-none"
          draggable={false}
          loading="eager"
        />
      </div>

      <label htmlFor={sliderId} className="sr-only">
        Compare Dark and Light theme screenshots
      </label>
      <input
        id={sliderId}
        type="range"
        min={0}
        max={100}
        step={1}
        value={position}
        aria-valuetext={`${position}% Dark theme, ${100 - position}% Light theme`}
        onChange={(event) => setPosition(Number(event.currentTarget.value))}
        className="theme-compare-range absolute inset-0 z-20 m-0 h-full w-full cursor-ew-resize opacity-0"
      />

      {/* Visual Divider & Handle */}
      <div
        aria-hidden="true"
        className="theme-compare-divider pointer-events-none absolute inset-y-0 z-10"
        style={{ left: `${position}%` }}
      >
        <div className="theme-compare-line h-full w-0.5 -translate-x-1/2 bg-fd-primary shadow-[0_0_0_1px_rgba(0,0,0,0.35)]" />
        <div className="theme-compare-handle absolute top-1/2 left-0 flex size-10 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-full border border-fd-border bg-fd-card text-fd-foreground shadow-md">
          <MoveHorizontal className="size-4" />
        </div>
      </div>
    </div>
  );
}
