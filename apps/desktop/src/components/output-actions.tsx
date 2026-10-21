import { useState } from 'react';
import { FolderOpen, Loader2, Play } from 'lucide-react';
import { errorMessage } from '@downpour/contracts';
import { bridge } from '../lib/bridge';
import { useApp } from '../state/app';
import { Button } from './ui/button';

export function OutputActions({ id }: { id: string }) {
  const [pending, setPending] = useState(false);
  async function open(reveal: boolean) {
    setPending(true);
    try {
      await bridge.invoke('open_output', { id, reveal });
    } catch (error) {
      useApp.setState({ error: errorMessage(error) });
    } finally {
      setPending(false);
    }
  }
  return (
    <div className="output-actions">
      <Button
        variant="outline"
        size="sm"
        disabled={pending}
        onClick={() => void open(false)}
        aria-label="Open saved file"
      >
        {pending ? <Loader2 size={13} className="spin" /> : <Play size={13} />}
        Open
      </Button>
      <Button
        variant="ghost"
        size="icon"
        disabled={pending}
        onClick={() => void open(true)}
        aria-label="Show saved file in folder"
        title="Show in folder"
      >
        <FolderOpen size={15} />
      </Button>
    </div>
  );
}
