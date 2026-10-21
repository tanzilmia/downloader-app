import { Clock3 } from 'lucide-react';
import { duration } from '../lib/utils';

export function RequestPause({
  seconds,
  id = 'youtube-pause',
}: {
  seconds: number;
  id?: string;
}) {
  if (seconds <= 0) return null;
  return (
    <div id={id} className="request-pause" role="status" aria-live="off">
      <Clock3 size={15} aria-hidden="true" />
      <p>
        <strong>YouTube requests paused · {duration(seconds)}</strong>
        <span>
          This pause reduces repeated requests. YouTube may still limit access
          afterward.
        </span>
      </p>
    </div>
  );
}
