import { useState } from 'react';
import {
  Check,
  Database,
  HardDrive,
  Laptop,
  Loader2,
  Moon,
  ShieldCheck,
  Sun,
} from 'lucide-react';
import {
  errorMessage,
  type Settings as AppSettings,
} from '@downpour/contracts';
import { useApp } from '../state/app';
import { Button } from '../components/ui/button';
import { DownloadFolder } from '../components/download-folder';

export function Settings() {
  const saved = useApp((state) => state.settings);
  const persistence = useApp((state) => state.persistence);
  const engine = useApp((state) => state.engine);
  const engineInfo = useApp((state) => state.engineInfo);
  const saveSettings = useApp((state) => state.saveSettings);
  const [draft, setDraft] = useState(saved);
  const [pending, setPending] = useState(false);
  const [choosingFolder, setChoosingFolder] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const dirty =
    draft.concurrency !== saved.concurrency || draft.theme !== saved.theme;
  async function save() {
    setPending(true);
    setNotice(null);
    setError(null);
    try {
      await saveSettings(draft);
      setNotice('Preferences saved on this device.');
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setPending(false);
    }
  }
  function update(value: Partial<AppSettings>) {
    setDraft((current) => ({ ...current, ...value }));
    setNotice(null);
    setError(null);
  }
  return (
    <div className="page settings-page">
      <div className="page-top">
        <span className="eyebrow">MAKE YOURSELF AT HOME</span>
        <span className="local-label">
          v0.1.0 · {engine === 'mock' ? 'Browser preview' : 'Desktop'}
        </span>
      </div>
      <div className="screen-heading">
        <div>
          <h1>Settings.</h1>
          <p>Make Downpour work the way you like.</p>
        </div>
      </div>
      <section className="settings-section">
        <div className="settings-intro">
          <h2>Download location</h2>
          <p>Choose where your videos and audio are saved.</p>
        </div>
        <DownloadFolder
          disabled={pending}
          onPendingChange={setChoosingFolder}
          onChange={(settings) =>
            setDraft((current) => ({
              ...current,
              downloadDirectory: settings.downloadDirectory,
            }))
          }
        />
      </section>
      <section className="settings-section">
        <div className="settings-intro">
          <h2>Look & feel</h2>
          <p>A comfortable corner, any time of day.</p>
        </div>
        <fieldset className="theme-picker">
          <legend className="sr-only">Appearance</legend>
          {(
            [
              { value: 'light', label: 'Light', icon: Sun },
              { value: 'dark', label: 'Dark', icon: Moon },
              { value: 'system', label: 'System', icon: Laptop },
            ] as const
          ).map(({ value, label, icon: Icon }) => (
            <label
              className={`theme-option ${draft.theme === value ? 'selected' : ''}`}
              key={value}
            >
              <input
                type="radio"
                name="theme"
                value={value}
                checked={draft.theme === value}
                onChange={() => update({ theme: value })}
                disabled={pending}
              />
              <div className={`theme-preview theme-preview-${value}`}>
                <div />
                <span />
                <span />
              </div>
              <span>
                <Icon size={15} />
                {label}
                {draft.theme === value ? <Check size={14} /> : null}
              </span>
            </label>
          ))}
        </fieldset>
      </section>
      <section className="settings-section">
        <div className="settings-intro">
          <h2>At your pace</h2>
          <p>Choose how many transfers run together.</p>
        </div>
        <label className="concurrency-setting">
          <span>
            Simultaneous downloads
            <small>New jobs wait when all slots are occupied.</small>
          </span>
          <select
            aria-label="Simultaneous downloads"
            value={draft.concurrency}
            onChange={(event) =>
              update({ concurrency: Number(event.target.value) })
            }
            disabled={pending}
          >
            {[1, 2, 3, 4].map((value) => (
              <option key={value} value={value}>
                {value} at a time
              </option>
            ))}
          </select>
        </label>
      </section>
      <section className="settings-section">
        <div className="settings-intro">
          <h2>Right here, on your device</h2>
          <p>Private by design. Simple by choice.</p>
        </div>
        <div className="info-rows">
          <div>
            <Database size={19} />
            <span>
              Local persistence
              <small>
                {persistence === 'sqlite'
                  ? 'SQLite with versioned migrations in your app-data directory.'
                  : 'Browser preview uses localStorage. The desktop app uses SQLite.'}
              </small>
            </span>
            <span className="info-pill">
              {persistence === 'sqlite' ? 'SQLITE' : 'BROWSER'}
            </span>
          </div>
          <div>
            <HardDrive size={19} />
            <span>
              Download engine
              <small>{engineInfo.message}</small>
            </span>
            <span className="info-pill">
              {engine === 'mock'
                ? 'MOCK'
                : engineInfo.available
                  ? 'READY'
                  : 'UNAVAILABLE'}
            </span>
          </div>
          {engineInfo.tools.length ? (
            <div className="engine-versions">
              <span>
                Media tools
                <small>
                  {engineInfo.tools
                    .map((tool) => `${tool.name}: ${tool.version}`)
                    .join('\n')}
                </small>
              </span>
            </div>
          ) : null}
          <div>
            <ShieldCheck size={19} />
            <span>
              Your space stays yours
              <small>
                No account, cloud database, tracking or browser cookie
                collection.
              </small>
            </span>
          </div>
        </div>
      </section>
      <div className="settings-actions">
        <div aria-live="polite">
          {notice ? (
            <p className="success-text">
              <Check size={14} />
              {notice}
            </p>
          ) : error ? (
            <p role="alert" className="field-error">
              {error}
            </p>
          ) : (
            <p>Preferences are saved locally.</p>
          )}
        </div>
        <Button
          disabled={!dirty || pending || choosingFolder}
          onClick={() => void save()}
        >
          {pending ? <Loader2 className="spin" size={16} /> : null}
          {pending ? 'Saving…' : 'Save preferences'}
        </Button>
      </div>
    </div>
  );
}
