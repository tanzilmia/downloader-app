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
  MonitorPlay,
  Play,
  Plus,
  Sparkles,
  X,
} from 'lucide-react';
import {
  detectPlatform,
  errorMessage,
  publicUrlSchema,
  type Media,
} from '@downpour/contracts';
import { bridge } from '../lib/bridge';
import { bytes, duration } from '../lib/utils';
import { useApp } from '../state/app';
import { Button } from '../components/ui/button';
import { MediaArt } from '../components/media-art';
import { JobList } from '../components/job-list';
import { EmptyState } from '../components/empty-state';
import { RequestPause } from '../components/request-pause';
import { useRetryPause } from '../lib/use-retry-pause';

function PreviewDialog({
  media,
  onClose,
}: {
  media: Media | null;
  onClose: () => void;
}) {
  const formats =
    media?.formats.filter((format) => format.extension !== 'webm') ?? [];
  const [selected, setSelected] = useState(formats[0]?.id ?? '');
  const selectedFormat = formats.find((format) => format.id === selected);
  const formatGroups = [
    { title: 'Audio only', icon: AudioLines, audioOnly: true },
    { title: 'Video', icon: MonitorPlay, audioOnly: false },
  ];
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const upsert = useApp((state) => state.upsert);
  const navigate = useNavigate();
  const pause = useRetryPause();
  async function enqueue() {
    if (
      !media ||
      pending ||
      pause.seconds ||
      !formats.some((format) => format.id === selected)
    )
      return;
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
      pause.capture(error);
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
            {media?.isMock
              ? 'Choose a format for this mock transfer.'
              : 'Choose a format to save on this device.'}
          </Dialog.Description>
          {media ? (
            <>
              <div className="preview-media">
                <MediaArt
                  platform={media.platform}
                  audio={selectedFormat?.audioOnly}
                />
                <span className="duration-tag">
                  {duration(media.durationSeconds)}
                </span>
              </div>
              <h3 className="preview-title">{media.title}</h3>
              <p className="preview-creator">
                {media.creator} <span>·</span> {media.platform}
              </p>
              {formats.length ? (
                <fieldset className="format-options">
                  <legend>DOWNLOAD FORMAT</legend>
                  {formatGroups.map(({ title, icon: Icon, audioOnly }) => (
                    <div className="format-group" key={title}>
                      <h4>
                        <Icon size={15} />
                        {title}
                        <span>{audioOnly ? 'M4A' : 'MP4'}</span>
                      </h4>
                      {formats
                        .filter((format) => format.audioOnly === audioOnly)
                        .map((format) => (
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
                              {selected === format.id ? (
                                <Check size={12} />
                              ) : null}
                            </span>
                          </label>
                        ))}
                      {!formats.some(
                        (format) => format.audioOnly === audioOnly,
                      ) ? (
                        <p className="format-unavailable">
                          {audioOnly
                            ? 'No compatible M4A audio is available for this link.'
                            : 'This link has no MP4 video option.'}
                        </p>
                      ) : null}
                    </div>
                  ))}
                </fieldset>
              ) : (
                <p className="field-error" role="status">
                  No MP4 or M4A download options are available for this link.
                  Try another public link.
                </p>
              )}
              {selectedFormat?.id.startsWith('audio:') ? (
                <p className="audio-extraction-note">
                  Audio is extracted from the source video. The video must be
                  downloaded first; only the M4A audio file is saved.
                </p>
              ) : null}
              <p className="mock-note">
                <Sparkles size={14} />
                {media.isMock
                  ? 'Mock preview. Metadata and transfers are simulated; no media file will be saved.'
                  : 'Saved locally after media and integrity checks. Keep only media you have permission to download.'}
              </p>
              {error ? (
                <p role="alert" className="field-error">
                  {error}
                </p>
              ) : null}
              <RequestPause
                seconds={pause.seconds}
                id="preview-youtube-pause"
              />
              <Button
                className="w-full"
                data-testid="enqueue"
                onClick={() => void enqueue()}
                disabled={pending || pause.seconds > 0 || !formats.length}
                aria-describedby={
                  pause.seconds ? 'preview-youtube-pause' : undefined
                }
              >
                {pending ? (
                  <Loader2 size={17} className="spin" />
                ) : (
                  <Plus size={17} />
                )}{' '}
                {pending
                  ? 'Adding to queue…'
                  : pause.seconds
                    ? `Retry in ${duration(pause.seconds)}`
                    : media.isMock
                      ? 'Add mock download'
                      : selectedFormat?.audioOnly
                        ? 'Download audio'
                        : 'Download video'}
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
  const engine = useApp((state) => state.engine);
  const engineInfo = useApp((state) => state.engineInfo);
  const pause = useRetryPause();
  const parsedUrl = publicUrlSchema.safeParse(url);
  const pauseSeconds =
    parsedUrl.success && detectPlatform(parsedUrl.data) === 'youtube'
      ? pause.seconds
      : 0;
  async function inspect(event: FormEvent) {
    event.preventDefault();
    if (pending || pauseSeconds) return;
    setError(null);
    const parsed = publicUrlSchema.safeParse(url);
    if (!parsed.success) {
      setError(parsed.error.issues[0]?.message ?? 'Check your video link.');
      return;
    }
    setPending(true);
    try {
      setMedia(await bridge.invoke('inspect_media', { url: parsed.data }));
      if (ready && !engineInfo.available) {
        const snapshot = await bridge.invoke('get_snapshot', {});
        useApp.setState({ engineInfo: snapshot.engineInfo });
      }
    } catch (error) {
      pause.capture(error);
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
              aria-describedby={
                pauseSeconds
                  ? 'youtube-pause'
                  : error
                    ? 'url-error'
                    : 'link-help'
              }
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
            <Button
              type="submit"
              disabled={pending || pauseSeconds > 0}
              aria-describedby={pauseSeconds ? 'youtube-pause' : undefined}
            >
              {pending ? <Loader2 size={17} className="spin" /> : null}
              {pending
                ? 'Looking…'
                : pauseSeconds
                  ? `Retry in ${duration(pauseSeconds)}`
                  : 'Find video'}
              <ArrowRight size={16} />
            </Button>
          </div>
          {error ? (
            <p id="url-error" role="alert" className="field-error">
              {error}
            </p>
          ) : null}
          <RequestPause seconds={pauseSeconds} />
        </form>
        <div className="link-helper">
          <p id="link-help">A public link is all you need.</p>
          <button
            className="text-button"
            onClick={() => {
              setUrl(
                engine === 'mock'
                  ? 'https://www.youtube.com/watch?v=BaW_jenozKc'
                  : 'https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4',
              );
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
      </div>
      <footer className="page-footer">
        <span>
          <Sparkles size={13} />
          {engine === 'mock'
            ? 'Foundation preview · mock engine'
            : 'Local media engine · verified downloads'}
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
