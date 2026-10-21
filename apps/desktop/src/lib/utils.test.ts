import { describe, expect, it } from 'vitest';
import { transferRate } from './utils';

describe('download speed', () => {
  it.each([
    [undefined, '—'],
    [NaN, '—'],
    [-1, '—'],
    [0, '0 B/s'],
    [850, '850 B/s'],
    [128_000, '128.0 KB/s'],
    [1_250_000, '1.3 MB/s'],
  ])('formats %s bytes per second as %s', (value, expected) => {
    expect(transferRate(value)).toBe(expected);
  });
});
