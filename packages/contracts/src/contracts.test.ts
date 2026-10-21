import { describe, expect, it } from 'vitest';
import fixture from '../fixtures/job.json';
import realFixture from '../fixtures/real-job.json';
import urls from '../fixtures/urls.json';
import {
  canTransition,
  errorSchema,
  detectPlatform,
  jobSchema,
  publicUrlSchema,
  settingsSchema,
} from './index';

describe('wire contracts', () => {
  it('retains a typed local retry pause and still accepts older errors', () => {
    const error = {
      code: 'rate_limited',
      message: 'HTTP 429',
      retryable: true,
      retryAfterSeconds: 60,
    };
    expect(errorSchema.parse(error)).toEqual(error);
    expect(
      errorSchema.parse({
        code: 'network',
        message: 'Offline',
        retryable: true,
      }).retryAfterSeconds,
    ).toBeUndefined();
    for (const retryAfterSeconds of [0, -1, 901, Infinity, '60']) {
      expect(
        errorSchema.safeParse({ ...error, retryAfterSeconds }).success,
      ).toBe(false);
    }
  });
  it('validates the shared Rust/TypeScript job fixture without dropping fields', () => {
    expect(jobSchema.parse(fixture)).toEqual(fixture);
  });
  it('retains verified real output metadata and rejects a bad checksum', () => {
    expect(jobSchema.parse(realFixture)).toEqual(realFixture);
    expect(
      jobSchema.safeParse({
        ...realFixture,
        output: { ...realFixture.output, sha256: 'unverified' },
      }).success,
    ).toBe(false);
  });
  it('accepts optional live speed and rejects invalid rates while preserving old jobs', () => {
    expect(
      jobSchema.parse({ ...fixture, speedBytesPerSecond: 1_250_000 })
        .speedBytesPerSecond,
    ).toBe(1_250_000);
    expect(jobSchema.parse(fixture).speedBytesPerSecond).toBeUndefined();
    for (const speedBytesPerSecond of [-1, Infinity, NaN, '2 MB/s', null]) {
      expect(
        jobSchema.safeParse({ ...fixture, speedBytesPerSecond }).success,
      ).toBe(false);
    }
  });
  it.each(urls.valid)('accepts public URL %s', (url) => {
    expect(publicUrlSchema.safeParse(url).success).toBe(true);
  });
  it.each(urls.invalid)('rejects unsafe URL %s', (url) => {
    expect(publicUrlSchema.safeParse(url).success).toBe(false);
  });
  it('uses domain boundaries for platform names', () => {
    expect(detectPlatform('https://youtube.com.evil.org')).toBe('other');
  });
  it('prevents terminal completion from being retried', () => {
    expect(canTransition('completed', 'queued')).toBe(false);
    expect(canTransition('cancelled', 'queued')).toBe(true);
  });
  it('rejects excessive concurrency and invalid themes', () => {
    expect(
      settingsSchema.safeParse({ concurrency: 5, theme: 'dark' }).success,
    ).toBe(false);
    expect(
      settingsSchema.safeParse({ concurrency: 2, theme: 'custom' }).success,
    ).toBe(false);
  });
});
