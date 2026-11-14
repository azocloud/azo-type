'use client';

import { useId, useState } from 'react';
import Image from 'next/image';
import { MoveHorizontal } from 'lucide-react';
import darkScreenshot from '@/assets/screenshot.png';
import lightScreenshot from '@/assets/screenshot-light.png';

const PRESETS = [
  { label: 'Dark', value: 100 },
  { label: 'Split', value: 50 },
  { label: 'Light', value: 0 },
] as const;

export function ThemeCompareSlider() {
  const [position, setPosition] = useState(50);
  const sliderId = useId();

  return (
    <div>
      <div className="relative overflow-hidden select-none">
        {/* Base Layer: Light theme screenshot */}
        <Image
          src={lightScreenshot}
          alt="Azo Type desktop application window comparing Dark theme on the left and Light theme on the right, showing the file tree, document outline, code editor, and live document preview."
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

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-fd-border bg-fd-secondary/30 px-4 py-2 text-xs text-fd-muted-foreground">
        <span>
          Drag divider to compare themes:{' '}
          <strong className="font-medium text-fd-foreground">
            {position}% Dark
          </strong>{' '}
          ·{' '}
          <strong className="font-medium text-fd-foreground">
            {100 - position}% Light
          </strong>
        </span>
        <div
          className="flex items-center gap-1.5"
          role="group"
          aria-label="Theme comparison presets"
        >
          {PRESETS.map((preset) => {
            const isActive = position === preset.value;
            return (
              <button
                key={preset.label}
                type="button"
                aria-pressed={isActive}
                onClick={() => setPosition(preset.value)}
                className={`inline-flex min-h-7 items-center rounded-md border px-2.5 py-1 text-xs font-medium transition-colors ${
                  isActive
                    ? 'border-fd-primary bg-fd-primary text-fd-primary-foreground'
                    : 'border-fd-border bg-fd-card text-fd-foreground hover:bg-fd-accent'
                }`}
              >
                {preset.label}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
