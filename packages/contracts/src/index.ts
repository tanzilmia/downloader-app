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
    'unsupported',
    'interrupted',
    'internal',
  ]),
  message: z.string(),
  retryable: z.boolean(),
});
export type AppError = z.infer<typeof errorSchema>;
export const jobSchema = z.object({
  id: z.string().uuid(),
  media: mediaSchema,
  format: formatSchema,
  status: statusSchema,
  progress: z.number().min(0).max(100),
  attempt: z.number().int().nonnegative(),
  revision: z.number().int().nonnegative(),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
  error: errorSchema.nullable(),
  isMock: z.boolean(),
});
export type Job = z.infer<typeof jobSchema>;
export const settingsSchema = z.object({
  concurrency: z.number().int().min(1).max(4),
  theme: z.enum(['light', 'dark', 'system']),
});
export type Settings = z.infer<typeof settingsSchema>;
export const defaultSettings: Settings = { concurrency: 2, theme: 'dark' };
export const snapshotSchema = z.object({
  jobs: z.array(jobSchema),
  settings: settingsSchema,
  engine: z.literal('mock'),
  persistence: z.enum(['sqlite', 'browser']),
});
export type Snapshot = z.infer<typeof snapshotSchema>;
export const JOB_EVENT = 'job:updated';
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
  subscribe(listener: (job: Job) => void): Promise<() => void>;
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
