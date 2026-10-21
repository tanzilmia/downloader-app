import { useEffect, useState } from 'react';
import { errorSchema } from '@downpour/contracts';

/** A local request pause; reaching zero does not imply the source unblocked us. */
export function useRetryPause() {
  const [deadline, setDeadline] = useState<number | null>(null);
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    if (deadline === null) return;
    const timer = window.setInterval(() => {
      const remaining = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
      setSeconds(remaining);
      if (!remaining) setDeadline(null);
    }, 250);
    return () => window.clearInterval(timer);
  }, [deadline]);
  function capture(error: unknown) {
    const parsed = errorSchema.safeParse(error);
    if (
      parsed.success &&
      parsed.data.code === 'rate_limited' &&
      parsed.data.retryAfterSeconds
    ) {
      setSeconds(parsed.data.retryAfterSeconds);
      setDeadline(Date.now() + parsed.data.retryAfterSeconds * 1000);
    }
  }
  return { seconds, capture };
}
