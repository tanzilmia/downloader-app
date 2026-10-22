import { z } from 'zod';

export const publicUrlSchema = z
  .string()
  .trim()
  .min(1, 'Paste a video link to get started.')
  .max(2048)
  .transform((value, ctx) => {
    try {
      const url = new URL(value);
      const host = url.hostname.toLowerCase();
      if (
        !['http:', 'https:'].includes(url.protocol) ||
        url.username ||
        url.password ||
        url.port ||
        !host.includes('.') ||
        !/^[a-z0-9.-]+$/.test(host) ||
        !/[a-z]$/.test(host) ||
        host
          .split('.')
          .some(
            (label) => !label || label.startsWith('-') || label.endsWith('-'),
          ) ||
        [
          'localhost',
          'local',
          'internal',
          'test',
          'invalid',
          'example',
          'onion',
          'home',
          'lan',
        ].some((suffix) => host === suffix || host.endsWith(`.${suffix}`))
      ) {
        throw new Error('invalid');
      }
      url.hash = '';
      return url.toString();
    } catch {
      ctx.addIssue({
        code: 'custom',
        message:
          'Use a public http or https video link, without sign-in details or a custom port.',
      });
      return z.NEVER;
    }
  });

export const platformSchema = z.enum([
  'youtube',
  'instagram',
  'facebook',
  'tiktok',
  'other',
]);
export type Platform = z.infer<typeof platformSchema>;
export function detectPlatform(value: string): Platform {
  const host = new URL(value).hostname.toLowerCase();
  const matches = (domain: string) =>
    host === domain || host.endsWith(`.${domain}`);
  if (matches('youtube.com') || matches('youtu.be')) return 'youtube';
  if (matches('instagram.com')) return 'instagram';
  if (matches('facebook.com') || matches('fb.watch')) return 'facebook';
  if (matches('tiktok.com')) return 'tiktok';
  return 'other';
}
export const formatSchema = z.object({
  id: z.string(),
  label: z.string(),
  extension: z.enum(['mp4', 'webm', 'm4a']),
  quality: z.string(),
  estimatedBytes: z.number().int().nonnegative(),
  audioOnly: z.boolean(),
});
export type MediaFormat = z.infer<typeof formatSchema>;
export const mediaSchema = z.object({
  url: publicUrlSchema,
  title: z.string(),
  creator: z.string(),
  platform: platformSchema,
  durationSeconds: z.number().int().nonnegative(),
  formats: z.array(formatSchema).min(1),
  isMock: z.boolean(),
});
export type Media = z.infer<typeof mediaSchema>;
export const statusSchema = z.enum([
  'queued',
  'downloading',
  'completed',
  'failed',
  'cancelled',
]);
export type JobStatus = z.infer<typeof statusSchema>;
export const errorSchema = z.object({
  code: z.enum([
    'validation',
    'storage',
    'network',
    'rate_limited',
    'unsupported',
    'interrupted',
    'internal',
    'dependency',
    'disk_full',
    'integrity',
    'timeout',
  ]),
  message: z.string(),
  retryable: z.boolean(),
  retryAfterSeconds: z.number().int().positive().max(900).optional(),
});
export type AppError = z.infer<typeof errorSchema>;
export const outputSchema = z.object({
  path: z.string(),
  bytes: z.number().int().positive(),
  sha256: z.string().regex(/^[a-f0-9]{64}$/),
  durationSeconds: z.number().positive(),
});
export const engineInfoSchema = z.object({
  available: z.boolean(),
  message: z.string(),
  outputDirectory: z.string().nullable(),
  tools: z.array(z.object({ name: z.string(), version: z.string() })),
});
export type EngineInfo = z.infer<typeof engineInfoSchema>;
export const jobSchema = z.object({
  id: z.string().uuid(),
  media: mediaSchema,
  format: formatSchema,
  status: statusSchema,
  progress: z.number().min(0).max(100),
  speedBytesPerSecond: z.number().finite().nonnegative().optional(),
  attempt: z.number().int().nonnegative(),
  revision: z.number().int().nonnegative(),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
  error: errorSchema.nullable(),
  isMock: z.boolean(),
  output: outputSchema.optional(),
  downloadDirectory: z.string().min(1).max(4096).optional(),
});
export type Job = z.infer<typeof jobSchema>;
export const settingsSchema = z.object({
  concurrency: z.number().int().min(1).max(4),
  theme: z.enum(['light', 'dark', 'system']),
  downloadDirectory: z.string().min(1).max(4096).optional(),
});
export type Settings = z.infer<typeof settingsSchema>;
export const defaultSettings: Settings = { concurrency: 2, theme: 'dark' };
export const snapshotSchema = z.object({
  jobs: z.array(jobSchema),
  settings: settingsSchema,
  engine: z.enum(['mock', 'yt-dlp']),
  persistence: z.enum(['sqlite', 'browser']),
  engineInfo: engineInfoSchema.default({
    available: true,
    message: 'Simulated transfers. No media files are created.',
    outputDirectory: null,
    tools: [],
  }),
  health: errorSchema.nullable().default(null),
});
export type Snapshot = z.infer<typeof snapshotSchema>;
export const JOB_EVENT = 'job:updated';
export const HEALTH_EVENT = 'engine:health';
export const enqueueSchema = z.object({
  url: publicUrlSchema,
  formatId: z.string().min(1).max(100),
});
export const idSchema = z.object({ id: z.string().uuid() });
export const commandSchemas = {
  get_snapshot: { input: z.object({}), output: snapshotSchema },
  inspect_media: {
    input: z.object({ url: publicUrlSchema }),
    output: mediaSchema,
  },
  enqueue_job: { input: enqueueSchema, output: jobSchema },
  cancel_job: { input: idSchema, output: jobSchema },
  retry_job: { input: idSchema, output: jobSchema },
  update_settings: { input: settingsSchema, output: settingsSchema },
  choose_download_directory: {
    input: z.object({ reset: z.boolean() }).strict(),
    output: snapshotSchema.nullable(),
  },
  open_output: {
    input: idSchema.extend({ reveal: z.boolean() }),
    output: z.null(),
  },
};
export type CommandName = keyof typeof commandSchemas;
export type CommandInput<K extends CommandName> = z.input<
  (typeof commandSchemas)[K]['input']
>;
export type CommandOutput<K extends CommandName> = z.output<
  (typeof commandSchemas)[K]['output']
>;
export interface Bridge {
  invoke<K extends CommandName>(
    command: K,
    input: CommandInput<K>,
  ): Promise<CommandOutput<K>>;
  subscribe(
    listener: (job: Job) => void,
    onHealth?: (error: AppError | null) => void,
  ): Promise<() => void>;
}
export function errorMessage(error: unknown): string {
  if (error instanceof z.ZodError)
    return error.issues[0]?.message ?? 'The response could not be validated.';
  const parsed = errorSchema.safeParse(error);
  if (parsed.success) return parsed.data.message;
  return error instanceof Error
    ? error.message
    : 'Something went wrong. Please try again.';
}
export function canTransition(from: JobStatus, to: JobStatus): boolean {
  const allowed: Record<JobStatus, JobStatus[]> = {
    queued: ['downloading', 'cancelled'],
    downloading: ['completed', 'failed', 'cancelled', 'queued'],
    completed: [],
    failed: ['queued'],
    cancelled: ['queued'],
  };
  return allowed[from].includes(to);
}
