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
import { bytes, transferRate } from '../lib/utils';
import { Button } from './ui/button';
import { MediaArt } from './media-art';
import { OutputActions } from './output-actions';

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
  const statusLabel = job.isMock
    ? labels[job.status]
    : job.status === 'downloading'
      ? 'Downloading'
      : job.status === 'completed'
        ? 'Saved'
        : labels[job.status];
  return (
    <article
      className={`job-row job-${job.status}`}
      aria-label={`${job.media.title}, ${statusLabel}`}
    >
      <MediaArt
        compact
        platform={job.media.platform}
        audio={job.format.audioOnly}
        extension={job.format.extension}
      />
      <div className="job-details">
        <h3 title={job.media.title}>{job.media.title}</h3>
        <p>
          {job.media.creator}
          <span>·</span>
          {job.format.quality}
          <span>·</span>
          {job.format.extension.toUpperCase()}
          <span>·</span>
          {bytes(job.output?.bytes ?? job.format.estimatedBytes)}
          {job.output ? '' : ' est.'}
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
        {job.status === 'downloading' && !job.isMock ? (
          <p
            className="job-transfer"
            aria-label={`Download speed for ${job.media.title}`}
          >
            {job.progress >= 88 ? (
              <span>Processing file…</span>
            ) : (
              <>
                <ArrowDown size={12} aria-hidden="true" />
                <strong>{transferRate(job.speedBytesPerSecond)}</strong>
                <span>
                  {job.speedBytesPerSecond === undefined
                    ? 'Waiting for speed…'
                    : 'download speed'}
                </span>
              </>
            )}
          </p>
        ) : null}
        {job.error ? <p className="job-error">{job.error.message}</p> : null}
      </div>
      <div className="job-actions">
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
              : statusLabel}
          </span>
        </div>
        {active ? (
          <Button
            variant="ghost"
            size="icon"
            disabled={pending}
            onClick={() => void cancel(job.id)}
            aria-label={`Cancel ${job.media.title}`}
            title={job.isMock ? 'Cancel simulation' : 'Cancel download'}
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
            title={job.isMock ? 'Retry simulation' : 'Retry download'}
          >
            <RotateCcw size={17} />
          </Button>
        ) : job.output ? (
          <OutputActions id={job.id} />
        ) : (
          <span
            className="job-done"
            title="Simulation completed. No media file was created."
          >
            <Check size={17} />
          </span>
        )}
      </div>
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
