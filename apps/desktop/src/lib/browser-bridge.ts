import {
  canTransition,
  commandSchemas,
  defaultSettings,
  snapshotSchema,
  type Bridge,
  type CommandInput,
  type CommandName,
  type CommandOutput,
  type Job,
  type JobStatus,
  type Snapshot,
} from '@downpour/contracts';
import { mockMedia } from './mock-media';

export const STORAGE_KEY = 'downpour.browser.v1';
type StoragePort = Pick<Storage, 'getItem' | 'setItem'>;

/** Development adapter only. The native application uses the Rust/SQLite core. */
export class BrowserBridge implements Bridge {
  private state: Snapshot | undefined;
  private listeners = new Set<(job: Job) => void>();
  private timer: ReturnType<typeof setInterval> | undefined;
  constructor(
    private storage: StoragePort,
    private intervalMs = 350,
  ) {}

  private load() {
    if (this.state) return;
    const raw = this.storage.getItem(STORAGE_KEY);
    const snapshot = raw
      ? snapshotSchema.parse(JSON.parse(raw))
      : {
          jobs: [],
          settings: { ...defaultSettings },
          engine: 'mock' as const,
          persistence: 'browser' as const,
          engineInfo: {
            available: true,
            message: 'Simulated transfers. No media files are created.',
            outputDirectory: null,
            tools: [],
          },
          health: null,
        };
    for (const job of snapshot.jobs) {
      if (job.status === 'downloading') {
        this.transition(job, 'queued');
        job.progress = 0;
        job.error = {
          code: 'interrupted',
          message:
            'Previous preview ended during this transfer. Queued for a fresh attempt.',
          retryable: true,
        };
      }
    }
    this.persist(snapshot);
    this.state = snapshot;
  }

  private persist(snapshot: Snapshot) {
    try {
      this.storage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
    } catch {
      throw new Error(
        'Browser storage could not be saved. Check available storage or use the desktop app.',
      );
    }
  }

  private change(mutate: (next: Snapshot) => void) {
    this.load();
    const next = structuredClone(this.state!);
    mutate(next);
    this.persist(next);
    const previous = this.state!;
    this.state = next;
    for (const job of next.jobs) {
      if (
        previous.jobs.find((item) => item.id === job.id)?.revision !==
        job.revision
      ) {
        for (const listener of this.listeners) listener(structuredClone(job));
      }
    }
    return next;
  }

  private transition(job: Job, next: JobStatus) {
    if (!canTransition(job.status, next))
      throw new Error(`Cannot change ${job.status} to ${next}.`);
    job.status = next;
    delete job.speedBytesPerSecond;
    job.revision++;
    job.updatedAt = new Date().toISOString();
  }

  tick() {
    this.change((next) => {
      let slots = Math.max(
        0,
        next.settings.concurrency -
          next.jobs.filter((job) => job.status === 'downloading').length,
      );
      for (const job of next.jobs) {
        if (job.status === 'downloading') {
          job.progress = Math.min(100, job.progress + 5);
          job.revision++;
          job.updatedAt = new Date().toISOString();
          if (job.progress === 100) this.transition(job, 'completed');
        } else if (job.status === 'queued' && slots > 0) {
          this.transition(job, 'downloading');
          job.attempt++;
          job.error = null;
          slots--;
        }
      }
    });
  }

  async invoke<K extends CommandName>(
    command: K,
    input: CommandInput<K>,
  ): Promise<CommandOutput<K>> {
    this.load();
    let output: unknown;
    switch (command) {
      case 'open_output':
        throw new Error(
          'Saved files are available in the desktop application.',
        );
      case 'get_snapshot':
        output = this.state;
        break;
      case 'inspect_media': {
        const { url } = commandSchemas.inspect_media.input.parse(input);
        output = mockMedia(url);
        break;
      }
      case 'enqueue_job': {
        const { url, formatId } = commandSchemas.enqueue_job.input.parse(input);
        const media = mockMedia(url);
        const format = media.formats.find((item) => item.id === formatId);
        if (!format) throw new Error('Choose an available media format.');
        const timestamp = new Date().toISOString();
        const job: Job = {
          id: crypto.randomUUID(),
          media,
          format,
          status: 'queued',
          progress: 0,
          attempt: 0,
          revision: 0,
          createdAt: timestamp,
          updatedAt: timestamp,
          error: null,
          isMock: true,
        };
        this.change((next) => {
          next.jobs.push(job);
        });
        output = job;
        break;
      }
      case 'cancel_job':
      case 'retry_job': {
        const { id } = commandSchemas.cancel_job.input.parse(input);
        const next = this.change((snapshot) => {
          const job = snapshot.jobs.find((item) => item.id === id);
          if (!job) throw new Error('This download could not be found.');
          if (command === 'retry_job') {
            if (!['failed', 'cancelled'].includes(job.status))
              throw new Error(
                'Only failed or cancelled downloads can be retried.',
              );
            this.transition(job, 'queued');
            job.progress = 0;
            job.error = null;
          } else {
            this.transition(job, 'cancelled');
          }
        });
        output = next.jobs.find((job) => job.id === id);
        break;
      }
      case 'update_settings': {
        const settings = commandSchemas.update_settings.input.parse(input);
        this.change((next) => {
          next.settings = settings;
        });
        output = settings;
        break;
      }
    }
    return structuredClone(
      commandSchemas[command].output.parse(output),
    ) as CommandOutput<K>;
  }

  async subscribe(listener: (job: Job) => void) {
    this.load();
    this.listeners.add(listener);
    if (!this.timer)
      this.timer = setInterval(() => {
        try {
          if (
            this.state?.jobs.some(
              (job) => job.status === 'queued' || job.status === 'downloading',
            )
          )
            this.tick();
        } catch (error) {
          this.dispose();
          // Surface the failure to the global UI handler; never continue a nondurable queue.
          globalThis.dispatchEvent?.(
            new ErrorEvent('error', {
              error,
              message:
                error instanceof Error
                  ? error.message
                  : 'Preview storage failed.',
            }),
          );
        }
      }, this.intervalMs);
    return () => {
      this.listeners.delete(listener);
      if (!this.listeners.size) this.dispose();
    };
  }

  dispose() {
    clearInterval(this.timer);
    this.timer = undefined;
  }
}
