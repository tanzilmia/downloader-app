import { useEffect } from 'react';
import {
  HashRouter,
  Link,
  NavLink,
  Route,
  Routes,
  useLocation,
} from 'react-router';
import {
  ArrowDownToLine,
  ArrowUpRight,
  CircleHelp,
  Download,
  FolderHeart,
  Leaf,
  LockKeyhole,
  Plus,
  Settings2,
  Sparkles,
  X,
} from 'lucide-react';
import { useApp } from './state/app';
import { Home } from './pages/home';
import { Downloads } from './pages/downloads';
import { Library } from './pages/library';
import { Settings } from './pages/settings';
import { Button } from './components/ui/button';

function Shell() {
  const { pathname } = useLocation();
  const currentPage =
    pathname === '/downloads'
      ? 'Downloads'
      : pathname === '/library'
        ? 'Library'
        : pathname === '/settings'
          ? 'Settings'
          : 'Overview';
  const jobs = useApp((state) => state.jobs);
  const theme = useApp((state) => state.settings.theme);
  const ready = useApp((state) => state.ready);
  const persistence = useApp((state) => state.persistence);
  const error = useApp((state) => state.error);
  const engine = useApp((state) => state.engine);
  const engineInfo = useApp((state) => state.engineInfo);
  const health = useApp((state) => state.health);
  const dismissError = useApp((state) => state.dismissError);
  const activeCount = jobs.filter(
    (job) => job.status === 'queued' || job.status === 'downloading',
  ).length;
  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === 'system' ? (media.matches ? 'dark' : 'light') : theme;
    };
    apply();
    media.addEventListener('change', apply);
    return () => media.removeEventListener('change', apply);
  }, [theme]);
  return (
    <div className="app-shell">
      <a
        className="skip-link"
        href="#main-content"
        onClick={(event) => {
          // HashRouter owns the URL hash; move focus without changing routes.
          event.preventDefault();
          document.getElementById('main-content')?.focus();
        }}
      >
        Skip to content
      </a>
      <aside className="sidebar">
        <Link to="/" className="brand" aria-label="Downpour home">
          <span className="brand-icon">
            <ArrowDownToLine size={23} strokeWidth={2} />
          </span>
          <span>
            downpour<span className="brand-period">.</span>
          </span>
        </Link>
        <div className="sidebar-caption">Your offline workspace</div>
        <Button className="new-download-button" asChild>
          <Link to="/" aria-label="New download" title="New download">
            <Plus size={18} />
            <span>New download</span>
          </Link>
        </Button>
        <div className="nav-label">Workspace</div>
        <nav aria-label="Main navigation">
          <NavLink to="/" end aria-label="Overview" title="Overview">
            <Leaf size={18} />
            <span>Overview</span>
          </NavLink>
          <NavLink to="/downloads" aria-label="Downloads" title="Downloads">
            <Download size={18} />
            <span>Downloads</span>
            {activeCount ? (
              <span className="nav-count">{activeCount}</span>
            ) : null}
          </NavLink>
          <NavLink to="/library" aria-label="Library" title="Library">
            <FolderHeart size={18} />
            <span>Library</span>
          </NavLink>
        </nav>
        <div className="sidebar-bottom">
          <div className="local-card">
            <span>
              <LockKeyhole size={15} />
              Stored on this device
            </span>
            <div>
              <span className="status-dot" />
              {ready
                ? persistence === 'sqlite'
                  ? 'SQLite connected'
                  : 'Browser preview'
                : 'Connecting…'}
            </div>
          </div>
          <nav aria-label="Preferences">
            <NavLink to="/settings" aria-label="Settings" title="Settings">
              <Settings2 size={18} />
              <span>Settings</span>
            </NavLink>
          </nav>
          <div className="sidebar-version">
            <span>Downpour v0.1.0</span>
            <Leaf size={12} />
          </div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="topbar-breadcrumb">
            <span>Workspace</span>
            <span>/</span>
            <span>{currentPage}</span>
          </div>
          <div className="preview-pill">
            <Sparkles size={12} />
            {engine === 'mock' ? 'MOCK PREVIEW' : 'LOCAL DOWNLOADS'}
          </div>
          <Link
            className="topbar-help"
            to="/settings"
            aria-label="Help and app information"
          >
            <CircleHelp size={18} />
          </Link>
        </header>
        {error ? (
          <div className="app-error" role="alert">
            <span>{error}</span>
            {ready ? (
              <Button
                variant="ghost"
                size="icon"
                onClick={dismissError}
                aria-label="Dismiss error"
              >
                <X size={16} />
              </Button>
            ) : (
              <Button
                variant="outline"
                size="sm"
                onClick={() => window.location.reload()}
              >
                Retry startup
              </Button>
            )}
          </div>
        ) : null}
        {health || (ready && !engineInfo.available) ? (
          <div className="app-error" role="alert">
            <span>{health?.message ?? engineInfo.message}</span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => window.location.reload()}
            >
              Reconnect
            </Button>
          </div>
        ) : null}
        <main id="main-content" tabIndex={-1}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/downloads" element={<Downloads />} />
            <Route path="/library" element={<Library />} />
            <Route
              path="/settings"
              element={
                ready ? (
                  <Settings />
                ) : (
                  <div className="page">
                    <p>Loading local preferences…</p>
                  </div>
                )
              }
            />
            <Route
              path="*"
              element={
                <div className="page">
                  <h1>A little off the path.</h1>
                  <Button asChild>
                    <Link to="/">
                      Back to your space <ArrowUpRight size={16} />
                    </Link>
                  </Button>
                </div>
              }
            />
          </Routes>
        </main>
      </div>
    </div>
  );
}

export function App() {
  return (
    <HashRouter>
      <Shell />
    </HashRouter>
  );
}
