import { afterEach, describe, expect, it } from 'vitest';
import fixture from '../../../../packages/contracts/fixtures/job.json';
import { BrowserBridge, STORAGE_KEY } from './browser-bridge';
import { mockMedia } from './mock-media';

function memoryStorage() {
  const data = new Map<string, string>();
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => {
      data.set(key, value);
    },
  };
}
const instances: BrowserBridge[] = [];
function instance(storage = memoryStorage()) {
  const bridge = new BrowserBridge(storage);
  instances.push(bridge);
  return bridge;
}
async function enqueue(bridge: BrowserBridge) {
  return bridge.invoke('enqueue_job', {
    url: fixture.media.url,
    formatId: 'mock-1080p',
  });
}
afterEach(() => {
  for (const bridge of instances) bridge.dispose();
  instances.length = 0;
});

describe('browser preview adapter', () => {
  it('matches the shared native mock metadata', () => {
    expect(mockMedia(fixture.media.url)).toEqual(fixture.media);
  });
  it('bounds concurrency, cancels without later completion, and retries', async () => {
    const bridge = instance();
    await bridge.invoke('update_settings', { concurrency: 1, theme: 'dark' });
    const first = await enqueue(bridge);
    await enqueue(bridge);
    bridge.tick();
    expect(
      (await bridge.invoke('get_snapshot', {})).jobs.map((job) => job.status),
    ).toEqual(['downloading', 'queued']);
    await bridge.invoke('cancel_job', { id: first.id });
    for (let i = 0; i < 24; i++) bridge.tick();
    expect(
      (await bridge.invoke('get_snapshot', {})).jobs.map((job) => job.status),
    ).toEqual(['cancelled', 'completed']);
    await bridge.invoke('retry_job', { id: first.id });
    for (let i = 0; i < 24; i++) bridge.tick();
    const jobs = (await bridge.invoke('get_snapshot', {})).jobs;
    expect(jobs[0]).toMatchObject({
      status: 'completed',
      progress: 100,
      attempt: 2,
    });
    await expect(
      bridge.invoke('retry_job', { id: first.id }),
    ).rejects.toThrow();
  });
  it('persists settings/history and recovers interrupted jobs', async () => {
    const storage = memoryStorage();
    const first = instance(storage);
    await first.invoke('update_settings', { concurrency: 1, theme: 'light' });
    await enqueue(first);
    first.tick();
    first.tick();
    const next = instance(storage);
    const snapshot = await next.invoke('get_snapshot', {});
    expect(snapshot.settings).toEqual({ concurrency: 1, theme: 'light' });
    expect(snapshot.jobs[0]).toMatchObject({
      status: 'queued',
      progress: 0,
      attempt: 1,
      error: { code: 'interrupted' },
    });
  });
  it('does not publish or retain mutations when storage fails', async () => {
    const storage = memoryStorage();
    let fail = false;
    const bridge = instance({
      getItem: storage.getItem,
      setItem: (key, value) => {
        if (fail) throw new Error('quota');
        storage.setItem(key, value);
      },
    });
    const events: unknown[] = [];
    await bridge.subscribe((job) => events.push(job));
    fail = true;
    await expect(enqueue(bridge)).rejects.toThrow('storage');
    expect(events).toHaveLength(0);
    expect((await bridge.invoke('get_snapshot', {})).jobs).toHaveLength(0);
  });
  it('reports corrupt storage without discarding it', async () => {
    const storage = memoryStorage();
    storage.setItem(STORAGE_KEY, '{broken');
    await expect(
      instance(storage).invoke('get_snapshot', {}),
    ).rejects.toThrow();
    expect(storage.getItem(STORAGE_KEY)).toBe('{broken');
  });
  it('rejects unknown format IDs and unsafe links', async () => {
    const bridge = instance();
    await expect(
      bridge.invoke('enqueue_job', {
        url: fixture.media.url,
        formatId: '--exec',
      }),
    ).rejects.toThrow();
    await expect(
      bridge.invoke('inspect_media', { url: 'file:///etc/passwd' }),
    ).rejects.toThrow();
  });
});
