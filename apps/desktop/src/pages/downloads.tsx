import { useState } from 'react';
import {
  ArrowDownToLine,
  CheckCheck,
  Clock3,
  Gauge,
  Plus,
  ShieldCheck,
} from 'lucide-react';
import { Link } from 'react-router';
import { useApp } from '../state/app';
import { Button } from '../components/ui/button';
import { JobList } from '../components/job-list';
import { EmptyState } from '../components/empty-state';
import { transferRate } from '../lib/utils';

export function Downloads() {
  const jobs = useApp((state) => state.jobs);
  const concurrency = useApp((state) => state.settings.concurrency);
  const engine = useApp((state) => state.engine);
  const [filter, setFilter] = useState<'all' | 'active' | 'finished'>('all');
  const downloading = jobs.filter((job) => job.status === 'downloading');
  const queued = jobs.filter((job) => job.status === 'queued');
  const completed = jobs.filter((job) => job.status === 'completed');
  const active = jobs.filter(
    (job) => job.status === 'queued' || job.status === 'downloading',
  );
  const finished = jobs.filter(
    (job) => job.status !== 'queued' && job.status !== 'downloading',
  );
  const speeds = downloading.filter(
    (job) => !job.isMock && job.speedBytesPerSecond !== undefined,
  );
  const totalSpeed = speeds.reduce(
    (total, job) => total + job.speedBytesPerSecond!,
    0,
  );
  const waitingForSpeed =
    !speeds.length &&
    downloading.some((job) => !job.isMock && job.progress < 88);
  const visible = [
    ...(filter === 'active' ? active : filter === 'finished' ? finished : jobs),
  ].reverse();
  const counts = {
    all: jobs.length,
    active: active.length,
    finished: finished.length,
  };
  return (
    <div className="page downloads-page">
      <div className="screen-heading">
        <div>
          <h1>Your downloads.</h1>
          <p>Everything you’re saving, all in one place.</p>
        </div>
        <Button asChild>
          <Link to="/">
            <Plus size={17} />
            New download
          </Link>
        </Button>
      </div>
      <div className="queue-summary">
        <div className="queue-stat">
          <span>
            <ArrowDownToLine size={15} />
            {engine === 'mock' ? 'Simulating' : 'Downloading'}
          </span>
          <strong>
            {downloading.length}
            <small>{downloading.length === 1 ? 'file' : 'files'}</small>
          </strong>
        </div>
        <div className="queue-stat">
          <span>
            <Clock3 size={15} />
            In queue
          </span>
          <strong>
            {queued.length}
            <small>{queued.length === 1 ? 'file' : 'files'}</small>
          </strong>
        </div>
        <div className="queue-stat">
          <span>
            <CheckCheck size={15} />
            {engine === 'mock' ? 'Completed mocks' : 'Completed'}
          </span>
          <strong>
            {completed.length}
            <small>{completed.length === 1 ? 'file' : 'files'}</small>
          </strong>
        </div>
        {engine !== 'mock' ? (
          <div
            className="queue-stat queue-speed"
            aria-label="Total download speed"
            title="Combined transfer speed of your active downloads"
          >
            <span>
              <Gauge size={15} />
              Download speed
            </span>
            <strong>
              {transferRate(waitingForSpeed ? undefined : totalSpeed)}
            </strong>
          </div>
        ) : null}
      </div>
      <section className="downloads-panel" aria-label="Download history">
        <div className="section-heading queue-heading">
          <div className="tabs" aria-label="Filter downloads">
            {(['all', 'active', 'finished'] as const).map((value) => (
              <button
                key={value}
                className={filter === value ? 'active' : ''}
                aria-pressed={filter === value}
                onClick={() => setFilter(value)}
              >
                {value === 'all'
                  ? 'All downloads'
                  : value === 'active'
                    ? 'Active'
                    : 'Finished'}
                <span className="tab-count">{counts[value]}</span>
              </button>
            ))}
          </div>
          <span className="queue-limit">Up to {concurrency} at a time</span>
        </div>
        {visible.length ? (
          <>
            <div className="download-list-labels" aria-hidden="true">
              <span>File</span>
              <span>Status & actions</span>
            </div>
            <JobList jobs={visible} />
          </>
        ) : (
          <EmptyState
            icon={ArrowDownToLine}
            title={
              filter === 'active'
                ? 'All quiet here.'
                : filter === 'finished'
                  ? 'Nothing finished just yet.'
                  : 'Something good is on the way.'
            }
            description={
              filter === 'active'
                ? 'Your next link is a good place to start.'
                : engine === 'mock'
                  ? 'Add a link and try your first simulated download.'
                  : 'Add an authorized public link to start your first download.'
            }
            action
          />
        )}
      </section>
      <p className="footnote download-footnote">
        <ShieldCheck size={14} />
        {engine === 'mock'
          ? 'Preview mode. Transfers are simulated; no files are saved.'
          : 'Files are checked before saving. Interrupted downloads can be retried.'}
      </p>
    </div>
  );
}
