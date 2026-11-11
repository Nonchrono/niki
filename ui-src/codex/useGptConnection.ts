import { useEffect, useState } from 'react';
import { invoke } from '../lib/tauri';
import type { CodexStatus } from './CodexSettings';

export const GPT_CONNECTION_CHANGED = 'niki-gpt-connection-changed';
export const CODEX_PREFS_CHANGED = 'niki-codex-prefs-changed';

export function useGptConnection() {
  const [connected, setConnected] = useState(false);
  useEffect(() => {
    let active = true;
    let checking = false;
    let checkAgain = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const check = async () => {
      if (document.visibilityState === 'hidden') return;
      if (checking) { checkAgain = true; return; }
      checking = true;
      clearTimeout(timer);
      try {
        const status = await invoke<CodexStatus>('codex_status');
        if (!active) return;
        setConnected(status.connected);
        if (status.login_pending) timer = setTimeout(check, 2000);
      } catch { if (active) setConnected(false); }
      finally {
        checking = false;
        if (active && checkAgain) { checkAgain = false; void check(); }
      }
    };
    void check();
    window.addEventListener('focus', check);
    window.addEventListener(GPT_CONNECTION_CHANGED, check);
    document.addEventListener('visibilitychange', check);
    return () => {
      active = false; clearTimeout(timer);
      window.removeEventListener('focus', check);
      window.removeEventListener(GPT_CONNECTION_CHANGED, check);
      document.removeEventListener('visibilitychange', check);
    };
  }, []);
  return connected;
}
