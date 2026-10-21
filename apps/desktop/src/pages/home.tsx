import { useState, type FormEvent } from 'react';
import { Link, useNavigate } from 'react-router';
import * as Dialog from '@radix-ui/react-dialog';
import {
  ArrowDown,
  ArrowRight,
  ArrowUpRight,
  AudioLines,
  Check,
  ChevronRight,
  Clipboard,
  Film,
  FolderHeart,
  Globe2,
  Link2,
  Loader2,
  LockKeyhole,
  MonitorPlay,
  Play,
  Plus,
  Sparkles,
  X,
} from 'lucide-react';
import { errorMessage, publicUrlSchema, type Media } from '@downpour/contracts';
import { bridge } from '../lib/bridge';
import { bytes, duration } from '../lib/utils';
import { useApp } from '../state/app';
import { Button } from '../components/ui/button';
import { MediaArt } from '../components/media-art';
import { JobList } from '../components/job-list';
import { EmptyState } from '../components/empty-state';

function PreviewDialog({
  media,
  onClose,
}: {
  media: Media | null;
  onClose: () => void;
}) {
  const [selected, setSelected] = useState('mock-1080p');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const upsert = useApp((state) => state.upsert);
  const navigate = useNavigate();
  async function enqueue() {
    if (!media || pending) return;
    setPending(true);
    setError(null);
    try {
      const job = await bridge.invoke('enqueue_job', {
        url: media.url,
        formatId: selected,
      });
      upsert(job);
      onClose();
      navigate('/downloads');
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setPending(false);
    }
  }
  return (
    <Dialog.Root
      open={!!media}
      onOpenChange={(open) => {
        if (!open && !pending) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content"
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            document.getElementById('video-url')?.focus();
          }}
        >
          <div className="dialog-heading">
            <span className="eyebrow">A GOOD FIND</span>
            <Dialog.Close asChild>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Close preview"
                disabled={pending}
              >
                <X size={19} />
              </Button>
            </Dialog.Close>
          </div>
          <Dialog.Title>Make it yours to revisit.</Dialog.Title>
          <Dialog.Description>
            Choose a format for this mock transfer.
          </Dialog.Description>
          {media ? (
            <>
              <div className="preview-media">
                <MediaArt platform={media.platform} />
                <span className="duration-tag">
                  {duration(media.durationSeconds)}
                </span>
              </div>
              <h3 className="preview-title">{media.title}</h3>
              <p className="preview-creator">
                {media.creator} <span>·</span> {media.platform}
              </p>
              <fieldset className="format-options">
                <legend>YOUR FORMAT</legend>
                {media.formats.map((format) => (
                  <label
                    key={format.id}
                    className={`format-option ${selected === format.id ? 'selected' : ''}`}
                  >
                    <input
                      type="radio"
                      name="format"
                      value={format.id}
                      checked={selected === format.id}
                      onChange={() => setSelected(format.id)}
                      disabled={pending}
                    />
                    <span className="format-icon">
                      {format.audioOnly ? (
                        <AudioLines size={20} />
                      ) : (
                        <MonitorPlay size={20} />
                      )}
                    </span>
                    <span>
                      <strong>
                        {format.label} <span>{format.quality}</span>
                      </strong>
                      <small>
                        {format.extension.toUpperCase()} <span>·</span>{' '}
                        {bytes(format.estimatedBytes)} estimated
                      </small>
                    </span>
                    <span className="radio-indicator">
                      {selected === format.id ? <Check size={12} /> : null}
                    </span>
                  </label>
                ))}
              </fieldset>
              <p className="mock-note">
                <Sparkles size={14} /> Mock preview. Metadata and transfers are
                simulated; no media file will be saved.
              </p>
              {error ? (
                <p role="alert" className="field-error">
                  {error}
                </p>
              ) : null}
              <Button
                className="w-full"
                onClick={() => void enqueue()}
                disabled={pending}
              >
                {pending ? (
                  <Loader2 size={17} className="spin" />
                ) : (
                  <Plus size={17} />
                )}{' '}
                {pending ? 'Adding to queue…' : 'Add mock download'}
              </Button>
            </>
          ) : null}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function Home() {
  const [url, setUrl] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [media, setMedia] = useState<Media | null>(null);
  const jobs = useApp((state) => state.jobs);
  const ready = useApp((state) => state.ready);
  async function inspect(event: FormEvent) {
    event.preventDefault();
    if (pending) return;
    setError(null);
    const parsed = publicUrlSchema.safeParse(url);
    if (!parsed.success) {
      setError(parsed.error.issues[0]?.message ?? 'Check your video link.');
      return;
    }
    setPending(true);
    try {
      setMedia(await bridge.invoke('inspect_media', { url: parsed.data }));
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setPending(false);
    }
  }
  async function paste() {
    try {
      setUrl(await navigator.clipboard.readText());
      setError(null);
    } catch {
      setError(
        'Clipboard access is unavailable. Paste your link using ⌘V or Ctrl+V.',
      );
    }
  }
  const recent = [...jobs].reverse().slice(0, 3);
  return (
    <div className="page home-page">
      <div className="page-top">
        <span className="eyebrow">YOUR OFFLINE CORNER</span>
        <span className="local-label">
          <span className="status-dot" />
          All yours. All local.
        </span>
      </div>
      <section className="hero">
        <div className="hero-copy">
          <div className="hero-kicker">
            <span /> Less scrolling. More keeping.
          </div>
          <h1>
            Keep the
            <br />
            <em>good parts.</em>
            <span className="title-dot" aria-hidden="true">
              *
            </span>
          </h1>
          <p>
            The tutorial. The inspiration. That one video.
            <br />
            Bring your favorites a little closer to home.
          </p>
        </div>
        <div className="hero-art" aria-hidden="true">
          <div className="orbit orbit-one" />
          <div className="orbit orbit-two" />
          <span className="orbit-star star-one">✳</span>
          <span className="orbit-star star-two">+</span>
          <div className="floating-file file-back">
            <Film size={20} />
            <span>THE GOOD STUFF</span>
          </div>
          <div className="floating-file file-front">
            <div className="file-top">
              <span>KEEP FOREVER</span>
              <ArrowUpRight size={15} />
            </div>
            <div className="file-download">
              <ArrowDown size={50} strokeWidth={1.5} />
            </div>
            <div className="file-bottom">
              <span>offline looks good.</span>
              <span>↓</span>
            </div>
          </div>
          <div className="saved-tag">
            <Check size={13} /> A little more yours.
          </div>
        </div>
      </section>
      <section className="download-box" aria-labelledby="new-download-title">
        <div className="section-heading">
          <h2 id="new-download-title">
            <Link2 size={18} /> Start with a link
          </h2>
          <span className="small-tag">ONE LINK. YOUR CHOICE.</span>
        </div>
        <form onSubmit={(event) => void inspect(event)} noValidate>
          <div className={`url-field ${error ? 'field-invalid' : ''}`}>
            <Link2 size={20} />
            <label className="sr-only" htmlFor="video-url">
              Video URL
            </label>
            <input
              id="video-url"
              type="url"
              placeholder="Paste a video link here…"
              value={url}
              onChange={(event) => {
                setUrl(event.target.value);
                setError(null);
              }}
              aria-invalid={!!error}
              aria-describedby={error ? 'url-error' : 'link-help'}
              disabled={pending}
              autoComplete="off"
              spellCheck={false}
            />
            <Button
              variant="ghost"
              size="sm"
              type="button"
              onClick={() => void paste()}
              aria-label="Paste video link"
              disabled={pending}
            >
              <Clipboard size={15} />
              <span>Paste</span>
            </Button>
            <Button type="submit" disabled={pending || !ready}>
              {pending ? <Loader2 size={17} className="spin" /> : null}
              {pending ? 'Looking…' : 'Find video'}
              <ArrowRight size={16} />
            </Button>
          </div>
          {error ? (
            <p id="url-error" role="alert" className="field-error">
              {error}
            </p>
          ) : null}
        </form>
        <div className="link-helper">
          <p id="link-help">A public link is all you need.</p>
          <button
            className="text-button"
            onClick={() => {
              setUrl('https://www.youtube.com/watch?v=BaW_jenozKc');
              setError(null);
              document.getElementById('video-url')?.focus();
            }}
          >
            Try a sample link <ArrowUpRight size={13} />
          </button>
        </div>
        <div className="source-row">
          <span>PUBLIC LINKS</span>
          <div className="source">
            <span className="youtube-mark" aria-hidden="true">
              <Play size={10} fill="currentColor" />
            </span>
            YouTube
          </div>
          <div className="source">
            <span className="instagram-mark" aria-hidden="true">
              ◎
            </span>
            Instagram
          </div>
          <div className="source">
            <span className="tiktok-mark" aria-hidden="true">
              ♪
            </span>
            TikTok
          </div>
          <div className="source">
            <span className="facebook-mark" aria-hidden="true">
              f
            </span>
            Facebook
          </div>
          <div className="source source-more">
            <Globe2 size={14} />& more
          </div>
        </div>
      </section>
      <div className="home-lower">
        <section className="recent-section">
          <div className="section-heading">
            <h2>
              Recently added{' '}
              <span className="count-label">
                {jobs.length.toString().padStart(2, '0')}
              </span>
            </h2>
            <Link className="text-link" to="/downloads">
              View all <ChevronRight size={14} />
            </Link>
          </div>
          {recent.length ? (
            <JobList jobs={recent} />
          ) : (
            <EmptyState
              icon={FolderHeart}
              title="Your collection starts here."
              description="Add a link above. Your recent downloads will feel right at home."
            />
          )}
        </section>
        <aside className="little-note">
          <span className="note-asterisk" aria-hidden="true">
            ✳
          </span>
          <span className="eyebrow">A SMALL REMINDER</span>
          <h3>
            Good finds.
            <br />
            Quietly kept.
          </h3>
          <p>
            No accounts. No ads.
            <br />
            Just a space for your favorites.
          </p>
          <div>
            <LockKeyhole size={13} />
            <span>Built to stay on your device</span>
          </div>
        </aside>
      </div>
      <footer className="page-footer">
        <span>
          <Sparkles size={13} /> Foundation preview · mock engine
        </span>
        <span>Only keep media you have permission to download.</span>
      </footer>
      <PreviewDialog
        key={media?.url ?? 'closed'}
        media={media}
        onClose={() => setMedia(null)}
      />
    </div>
  );
}
