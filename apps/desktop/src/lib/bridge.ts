import { isTauri, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  commandSchemas,
  jobSchema,
  JOB_EVENT,
  HEALTH_EVENT,
  errorSchema,
  type Bridge,
  type CommandInput,
  type CommandName,
  type CommandOutput,
} from '@downpour/contracts';
import { BrowserBridge } from './browser-bridge';

const nativeBridge: Bridge = {
  async invoke<K extends CommandName>(
    command: K,
    input: CommandInput<K>,
  ): Promise<CommandOutput<K>> {
    const validated = commandSchemas[command].input.parse(input);
    const response = await invoke<unknown>(
      command,
      command === 'get_snapshot' ? {} : { input: validated },
    );
    return commandSchemas[command].output.parse(response) as CommandOutput<K>;
  },
  async subscribe(listener, onHealth) {
    const unlisten = await listen<unknown>(JOB_EVENT, (event) => {
      const result = jobSchema.safeParse(event.payload);
      if (result.success) listener(result.data);
      else
        window.dispatchEvent(
          new ErrorEvent('error', {
            message:
              'A native progress event could not be validated. Restart the app to reconnect.',
            error: result.error,
          }),
        );
    });
    try {
      const unlistenHealth = await listen<unknown>(HEALTH_EVENT, (event) => {
        const result = errorSchema.nullable().safeParse(event.payload);
        if (result.success) onHealth?.(result.data);
      });
      return () => {
        unlisten();
        unlistenHealth();
      };
    } catch (error) {
      unlisten();
      throw error;
    }
  },
};

export const bridge: Bridge = isTauri()
  ? nativeBridge
  : new BrowserBridge({
      getItem: (key) => localStorage.getItem(key),
      setItem: (key, value) => localStorage.setItem(key, value),
    });
