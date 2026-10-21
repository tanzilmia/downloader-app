import { useState } from 'react';
import { FolderHeart, Search, SlidersHorizontal } from 'lucide-react';
import { useApp } from '../state/app';
import { EmptyState } from '../components/empty-state';
import { MediaArt } from '../components/media-art';
import { bytes, dateLabel, duration } from '../lib/utils';
import { OutputActions } from '../components/output-actions';

export function Library() {
  const jobs = useApp((state) => state.jobs);
  const engine = useApp((state) => state.engine);
  const [search, setSearch] = useState('');
  const [kind, setKind] = useState('all');
  const completed = jobs.filter((job) => job.status === 'completed');
  const filtered = completed
    .filter(
      (job) =>
        `${job.media.title} ${job.media.creator}`
          .toLowerCase()
          .includes(search.toLowerCase()) &&
        (kind === 'all' ||
          (kind === 'audio' ? job.format.audioOnly : !job.format.audioOnly)),
    )
    .reverse();
  return (
    <div className="page">
      <div className="page-top">
        <span className="eyebrow">A PLACE FOR YOUR FAVORITES</span>
        <span className="local-label">
          <FolderHeart size={14} />
          {completed.length}{' '}
          {engine === 'mock' ? 'completed mocks' : 'completed downloads'}
        </span>
      </div>
      <div className="screen-heading">
        <div>
          <h1>Your library.</h1>
          <p>Your saved videos and audio, ready to revisit.</p>
        </div>
      </div>
      <div className="library-tools">
        <div className="search-field">
          <Search size={17} />
          <label htmlFor="library-search" className="sr-only">
            Search your library
          </label>
          <input
            id="library-search"
            placeholder="Find something you kept…"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        <label className="select-wrap">
          <SlidersHorizontal size={15} />
          <span className="sr-only">Media type</span>
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value)}
          >
            <option value="all">All media</option>
            <option value="video">Video</option>
            <option value="audio">Audio only</option>
          </select>
        </label>
      </div>
      {filtered.length ? (
        <div className="library-grid">
          {filtered.map((job) => (
            <article className="library-card" key={job.id}>
              <div className="library-art">
                <MediaArt
                  platform={job.media.platform}
                  audio={job.format.audioOnly}
                />
                <span className="duration-tag">
                  {duration(
                    job.output?.durationSeconds ?? job.media.durationSeconds,
                  )}
                </span>
              </div>
              <div className="library-card-body">
                <div className="library-card-meta">
                  <span>{job.media.platform}</span>
                  <span>{dateLabel(job.createdAt)}</span>
                </div>
                <h2>{job.media.title}</h2>
                <p>{job.media.creator}</p>
                <div className="library-card-bottom">
                  <span>
                    {job.format.quality} <span>·</span>{' '}
                    {bytes(job.output?.bytes ?? job.format.estimatedBytes)}
                    {job.output ? '' : ' est.'}
                  </span>
                  <span className="mock-badge">
                    {job.isMock ? 'SIMULATED' : 'VERIFIED'}
                  </span>
                </div>
                {job.output ? <OutputActions id={job.id} /> : null}
              </div>
            </article>
          ))}
        </div>
      ) : (
        <EmptyState
          icon={FolderHeart}
          title={
            completed.length
              ? 'Nothing matches this search.'
              : 'Room for the things you love.'
          }
          description={
            completed.length
              ? 'Try a different title, creator or media type.'
              : engine === 'mock'
                ? 'Completed mock downloads will appear here. Your collection is just a link away.'
                : 'Your completed downloads will appear here, ready to open offline.'
          }
          action={!completed.length}
        />
      )}
      <p className="footnote">
        {engine === 'mock'
          ? 'Mock history only. These entries have no saved media files.'
          : 'Verified files stay in your download folder. Earlier simulated entries are labelled separately.'}
      </p>
    </div>
  );
}
