import { ArrowDown, AudioLines, Play } from 'lucide-react';
import type { Platform } from '@downpour/contracts';

export function MediaArt({
  platform = 'youtube',
  compact = false,
  audio = false,
}: {
  platform?: Platform;
  compact?: boolean;
  audio?: boolean;
}) {
  return (
    <div
      className={`media-art art-${platform} ${compact ? 'art-compact' : ''}`}
      aria-hidden="true"
    >
      <svg viewBox="0 0 400 225" preserveAspectRatio="xMidYMid slice">
        <rect width="400" height="225" fill="var(--art-sky)" />
        <circle cx="300" cy="66" r="30" fill="var(--art-sun)" />
        <path
          d="M0 180 87 70 178 166 235 109 400 225H0Z"
          fill="var(--art-back)"
        />
        <path d="m0 225 120-92 68 54 81-49 131 87Z" fill="var(--art-front)" />
        <path d="m194 225 31-55 21-10-18 65" fill="var(--art-road)" />
      </svg>
      {!compact ? (
        <span className="art-stamp">
          SAMPLE ARTWORK <ArrowDown size={12} />
        </span>
      ) : null}
      <span className="art-play">
        {audio ? (
          <AudioLines size={compact ? 16 : 24} />
        ) : (
          <Play size={compact ? 13 : 21} fill="currentColor" />
        )}
      </span>
    </div>
  );
}
