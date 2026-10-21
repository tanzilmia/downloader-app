import { create } from 'zustand';
import {
  defaultSettings,
  errorMessage,
  type Job,
  type Settings,
  type Snapshot,
  type EngineInfo,
  type AppError,
} from '@downpour/contracts';
import { bridge } from '../lib/bridge';

interface AppState {
  jobs: Job[];
  settings: Settings;
  persistence: Snapshot['persistence'];
  ready: boolean;
  engine: Snapshot['engine'];
  engineInfo: EngineInfo;
  health: AppError | null;
  error: string | null;
  pending: string[];
  upsert: (job: Job) => void;
  dismissError: () => void;
  cancel: (id: string) => Promise<void>;
  retry: (id: string) => Promise<void>;
  saveSettings: (settings: Settings) => Promise<void>;
}

async function jobAction(action: 'cancel_job' | 'retry_job', id: string) {
  if (useApp.getState().pending.includes(id)) return;
  useApp.setState((state) => ({ pending: [...state.pending, id] }));
  try {
    useApp.getState().upsert(await bridge.invoke(action, { id }));
  } catch (error) {
    useApp.setState({ error: errorMessage(error) });
  } finally {
    useApp.setState((state) => ({
      pending: state.pending.filter((item) => item !== id),
    }));
  }
}

export const useApp = create<AppState>((set) => ({
  jobs: [],
  settings: defaultSettings,
  persistence: 'browser',
  ready: false,
  engine: 'mock',
  engineInfo: {
    available: true,
    message: 'Simulated transfers. No media files are created.',
    outputDirectory: null,
    tools: [],
  },
  health: null,
  error: null,
  pending: [],
  upsert: (job) =>
    set((state) => {
      const previous = state.jobs.find((item) => item.id === job.id);
      if (previous && previous.revision >= job.revision) return state;
      return {
        jobs: previous
          ? state.jobs.map((item) => (item.id === job.id ? job : item))
          : [...state.jobs, job],
      };
    }),
  dismissError: () => set({ error: null }),
  cancel: (id) => jobAction('cancel_job', id),
  retry: (id) => jobAction('retry_job', id),
  saveSettings: async (settings) => {
    const saved = await bridge.invoke('update_settings', settings);
    set({ settings: saved });
  },
}));

export async function initializeApp() {
  let unsubscribe: (() => void) | undefined;
  try {
    unsubscribe = await bridge.subscribe(
      (job) => useApp.getState().upsert(job),
      (health) => useApp.setState({ health }),
    );
    const snapshot = await bridge.invoke('get_snapshot', {});
    // Events may arrive while fetching; revisions protect newer committed state.
    for (const job of snapshot.jobs) useApp.getState().upsert(job);
    useApp.setState({
      settings: snapshot.settings,
      persistence: snapshot.persistence,
      ready: true,
      engine: snapshot.engine,
      engineInfo: snapshot.engineInfo,
      health: snapshot.health,
    });
  } catch (error) {
    unsubscribe?.();
    useApp.setState({ error: errorMessage(error), ready: false });
  }
  return unsubscribe;
}
