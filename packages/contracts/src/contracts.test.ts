import { describe, expect, it } from 'vitest';
import fixture from '../fixtures/job.json';
import urls from '../fixtures/urls.json';
import {
  canTransition,
  detectPlatform,
  jobSchema,
  publicUrlSchema,
  settingsSchema,
} from './index';

describe('wire contracts', () => {
  it('validates the shared Rust/TypeScript job fixture without dropping fields', () => {
    expect(jobSchema.parse(fixture)).toEqual(fixture);
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
