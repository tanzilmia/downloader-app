import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
export function bytes(value: number) {
  if (value <= 0) return 'Unknown size';
  return value < 1_000_000
    ? `${(value / 1_000).toFixed(0)} KB`
    : `${(value / 1_000_000).toFixed(1)} MB`;
}
export function transferRate(value: number | undefined) {
  if (value === undefined || !Number.isFinite(value) || value < 0) return '—';
  if (value < 1_000) return `${Math.round(value)} B/s`;
  if (value < 1_000_000) return `${(value / 1_000).toFixed(1)} KB/s`;
  return `${(value / 1_000_000).toFixed(1)} MB/s`;
}
export function duration(seconds: number) {
  if (seconds <= 0) return '—';
  const wholeSeconds = Math.floor(seconds);
  return `${Math.floor(wholeSeconds / 60)}:${String(wholeSeconds % 60).padStart(2, '0')}`;
}
export function dateLabel(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
  }).format(new Date(value));
}
