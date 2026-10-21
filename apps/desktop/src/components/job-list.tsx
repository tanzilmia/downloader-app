import {
  ArrowDown,
  Check,
  CircleSlash,
  Clock3,
  RotateCcw,
  X,
} from 'lucide-react';
import type { Job, JobStatus } from '@downpour/contracts';
import { useApp } from '../state/app';
import { bytes } from '../lib/utils';
import { Button } from './ui/button';
import { MediaArt } from './media-art';

const labels: Record<JobStatus, string> = {
  queued: 'In queue',
  downloading: 'Simulating',
  completed: 'Simulated',
  cancelled: 'Cancelled',
  failed: 'Needs attention',
};

function JobRow({ job }: { job: Job }) {
  const cancel = useApp((state) => state.cancel);
  const retry = useApp((state) => state.retry);
  const pending = useApp((state) => state.pending.includes(job.id));
  const active = job.status === 'queued' || job.status === 'downloading';
  return (
    <article
      className="job-row"
      aria-label={`${job.media.title}, ${labels[job.status]}`}
    >
      <MediaArt
        compact
        platform={job.media.platform}
        audio={job.format.audioOnly}
      />
      <div className="job-details">
        <h3>{job.media.title}</h3>
        <p>
          {job.media.creator}
          <span>·</span>
          {job.format.quality}
          <span>·</span>
          {job.format.extension.toUpperCase()}
          <span>·</span>
          {bytes(job.format.estimatedBytes)} est.
        </p>
        {job.status === 'downloading' ? (
          <div
            className="job-progress"
            role="progressbar"
            aria-label={`Progress for ${job.media.title}`}
            aria-valuenow={job.progress}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <span style={{ width: `${job.progress}%` }} />
          </div>
        ) : null}
        {job.error ? <p className="job-error">{job.error.message}</p> : null}
      </div>
      <div className={`job-status status-${job.status}`}>
        {job.status === 'completed' ? (
          <Check size={14} />
        ) : job.status === 'queued' ? (
          <Clock3 size={14} />
        ) : job.status === 'downloading' ? (
          <ArrowDown size={14} />
        ) : (
          <CircleSlash size={14} />
        )}
        <span>
          {job.status === 'downloading'
            ? `${Math.round(job.progress)}%`
            : labels[job.status]}
        </span>
      </div>
      {active ? (
        <Button
          variant="ghost"
          size="icon"
          disabled={pending}
          onClick={() => void cancel(job.id)}
          aria-label={`Cancel ${job.media.title}`}
          title="Cancel simulation"
        >
          <X size={17} />
        </Button>
      ) : job.status === 'cancelled' || job.status === 'failed' ? (
        <Button
          variant="ghost"
          size="icon"
          disabled={pending}
          onClick={() => void retry(job.id)}
          aria-label={`Retry ${job.media.title}`}
          title="Retry simulation"
        >
          <RotateCcw size={17} />
        </Button>
      ) : (
        <span
          className="job-done"
          title="Simulation completed. No media file was created."
        >
          <Check size={17} />
        </span>
      )}
    </article>
  );
}

export function JobList({ jobs }: { jobs: Job[] }) {
  return (
    <div className="job-list">
      {jobs.map((job) => (
        <JobRow key={job.id} job={job} />
      ))}
    </div>
  );
}
