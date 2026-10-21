import {
  detectPlatform,
  publicUrlSchema,
  type Media,
} from '@downpour/contracts';

export function mockMedia(input: string): Media {
  const url = publicUrlSchema.parse(input);
  const platform = detectPlatform(url);
  const details = {
    youtube: ['A little further from the everyday', 'The Open Road', 247],
    instagram: ['Small moments, worth keeping', 'Everyday stories', 42],
    tiktok: ['A moment of inspiration', 'The creative corner', 31],
    facebook: ['Stories from a slower life', 'Field Notes', 128],
    other: ['Something worth coming back to', 'An open corner of the web', 180],
  } as const;
  const [title, creator, durationSeconds] = details[platform];
  return {
    url,
    platform,
    title,
    creator,
    durationSeconds,
    isMock: true,
    formats: [
      {
        id: 'mock-1080p',
        label: 'Full HD',
        extension: 'mp4',
        quality: '1080p',
        estimatedBytes: 86_000_000,
        audioOnly: false,
      },
      {
        id: 'mock-720p',
        label: 'HD',
        extension: 'mp4',
        quality: '720p',
        estimatedBytes: 42_000_000,
        audioOnly: false,
      },
      {
        id: 'mock-audio',
        label: 'Audio only',
        extension: 'm4a',
        quality: '128 kbps',
        estimatedBytes: 4_000_000,
        audioOnly: true,
      },
    ],
  };
}
