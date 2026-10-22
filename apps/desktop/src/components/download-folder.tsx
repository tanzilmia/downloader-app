import { useState } from 'react';
import { FolderOpen, Loader2, RotateCcw } from 'lucide-react';
import { errorMessage, type Settings } from '@downpour/contracts';
import { bridge } from '../lib/bridge';
import { useApp } from '../state/app';
import { Button } from './ui/button';

export function DownloadFolder({
  compact = false,
  disabled = false,
  onChange,
  onPendingChange,
}: {
  compact?: boolean;
  disabled?: boolean;
  onChange?: (settings: Settings) => void;
  onPendingChange?: (pending: boolean) => void;
}) {
  const directory = useApp((state) => state.engineInfo.outputDirectory);
  const custom = useApp((state) => state.settings.downloadDirectory);
  const native = useApp((state) => state.persistence === 'sqlite');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function choose(reset: boolean) {
    if (pending || disabled || !native) return;
    setPending(true);
    onPendingChange?.(true);
    setError(null);
    setNotice(null);
    try {
      const snapshot = await bridge.invoke('choose_download_directory', {
        reset,
      });
      if (!snapshot) return;
      useApp.setState({
        settings: snapshot.settings,
        engineInfo: snapshot.engineInfo,
      });
      onChange?.(snapshot.settings);
      setNotice(
        reset
          ? 'Default download folder restored.'
          : 'Download folder updated.',
      );
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setPending(false);
      onPendingChange?.(false);
    }
  }

  return (
    <div
      className={`download-location ${compact ? 'download-location-compact' : ''}`}
    >
      <div className="download-location-row">
        <FolderOpen size={18} aria-hidden="true" />
        <div className="download-location-path">
          <span>{compact ? 'Save to' : 'Current folder'}</span>
          <p title={custom ?? directory ?? undefined}>
            {native
              ? (custom ?? directory ?? 'Default download folder')
              : 'Available in the desktop app'}
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          disabled={!native || disabled || pending}
          onClick={() => void choose(false)}
          aria-label="Choose download folder"
        >
          {pending ? <Loader2 size={13} className="spin" /> : null}
          {pending ? 'Choosing…' : compact ? 'Change' : 'Choose folder'}
        </Button>
        {custom ? (
          <Button
            variant="ghost"
            size={compact ? 'icon' : 'sm'}
            disabled={disabled || pending || !native}
            onClick={() => void choose(true)}
            aria-label="Use default download folder"
            title="Use default download folder"
          >
            <RotateCcw size={13} />
            {!compact ? 'Use default' : null}
          </Button>
        ) : null}
      </div>
      {!compact ? (
        <p className="download-location-help">
          {native
            ? 'Folder changes are saved immediately and apply to new downloads. Existing files and queued downloads keep their location.'
            : 'Browser preview simulates downloads and does not save media files.'}
        </p>
      ) : null}
      {error ? (
        <p className="field-error" role="alert">
          {error}
        </p>
      ) : null}
      <span className="download-location-notice" role="status">
        {notice}
      </span>
    </div>
  );
}
