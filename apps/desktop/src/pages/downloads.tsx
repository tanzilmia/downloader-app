import { useState } from 'react';
import { ArrowDownToLine, Plus } from 'lucide-react';
import { Link } from 'react-router';
import { useApp } from '../state/app';
import { Button } from '../components/ui/button';
import { JobList } from '../components/job-list';
import { EmptyState } from '../components/empty-state';

export function Downloads() {
  const jobs = useApp((state) => state.jobs);
  const concurrency = useApp((state) => state.settings.concurrency);
  const [filter, setFilter] = useState<'all' | 'active' | 'finished'>('all');
  const active = jobs.filter(
    (job) => job.status === 'queued' || job.status === 'downloading',
  );
  const visible = [
    ...(filter === 'active'
      ? active
      : filter === 'finished'
        ? jobs.filter(
            (job) => job.status !== 'queued' && job.status !== 'downloading',
          )
        : jobs),
  ].reverse();
  return (
    <div className="page">
      <div className="page-top">
        <span className="eyebrow">IN GOOD HANDS</span>
        <span className="local-label">
          <span className="status-dot" />
          {active.length} in progress
        </span>
      </div>
      <div className="screen-heading">
        <div>
          <h1>Your downloads.</h1>
          <p>A little patience. A lot to come back to.</p>
        </div>
        <Button asChild>
          <Link to="/">
            <Plus size={17} />
            New download
          </Link>
        </Button>
      </div>
      <div className="queue-summary">
        <div>
          <strong>
            {jobs
              .filter((job) => job.status === 'downloading')
              .length.toString()
              .padStart(2, '0')}
          </strong>
          <span>SIMULATING</span>
        </div>
        <div>
          <strong>
            {jobs
              .filter((job) => job.status === 'queued')
              .length.toString()
              .padStart(2, '0')}
          </strong>
          <span>IN QUEUE</span>
        </div>
        <div>
          <strong>
            {jobs
              .filter((job) => job.status === 'completed')
              .length.toString()
              .padStart(2, '0')}
          </strong>
          <span>COMPLETED MOCKS</span>
        </div>
        <p>
          Up to {concurrency} at a time.
          <br />
          <span>Everything stays on this device.</span>
        </p>
      </div>
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
            </button>
          ))}
        </div>
        <span className="small-tag">MOCK ENGINE</span>
      </div>
      {visible.length ? (
        <JobList jobs={visible} />
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
              : 'Add a link and try your first simulated download.'
          }
          action
        />
      )}
      <p className="footnote">
        This foundation preview simulates progress. No media files are
        downloaded or saved.
      </p>
    </div>
  );
}
