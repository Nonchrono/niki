import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { useApp } from '../app/AppContext';
import { SYNC_COMPLETED } from '../lib/tauri';
import { errorMessage } from '../lib/format';
import { CODEX_PREFS_CHANGED, useGptConnection } from './useGptConnection';

export type SourceRef = { id: string; journal_id: string; date: string; title: string };
export type ChatMessage = { id: string; role: 'user' | 'assistant'; content: string; kind: 'chat' | 'summary'; sources?: SourceRef[] };
export type MemoryModel = { installed: boolean; downloading: boolean; failed: boolean; downloaded_bytes: number; total_bytes: number };
export type Prefs = {
  consented: boolean; excluded_journals: string[]; model: string | null; default_model: string;
  model_fallback: boolean; background: boolean;
};
type Stage = 'searching' | 'writing' | null;
type SummaryTarget = {
  entryId: string; title: string; disabled: boolean; prepare: () => Promise<string | null>;
  /** Appends text to the end of the open diary; false when the editor cannot take it. */
  append?: (text: string) => boolean;
};
type ChatState = {
  connected: boolean; open: boolean; toggle: () => void; close: () => void;
  messages: ChatMessage[]; organizing: boolean; pending: string; busy: boolean; error: string; ready: boolean;
  stage: Stage; streamText: string;
  draft: string; setDraft: (value: string) => void; reload: () => Promise<void>;
  target: SummaryTarget | null; registerTarget: (target: SummaryTarget | null) => void;
  send: (text?: string) => Promise<void>; summarize: () => Promise<void>;
  cancel: () => void; regenerate: () => Promise<void>; clearConversation: () => Promise<void>;
  model: MemoryModel | null;
  prefs: Prefs | null; setConsent: (value: boolean) => Promise<void>;
  pinned: boolean; setPinned: (value: boolean) => void;
};
const Context = createContext<ChatState | null>(null);
export const useChat = () => { const value = useContext(Context); if (!value) throw Error('Missing ChatProvider'); return value; };

const PINNED_KEY = 'niki-chat-pinned';
const readPinned = () => { try { return localStorage.getItem(PINNED_KEY) === '1'; } catch { return false; } };

export function ChatProvider({ children }: { children: ReactNode }) {
  const { run, scheduleSync } = useApp();
  const connected = useGptConnection();
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [draft, setDraft] = useState('');
  const [pending, setPending] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [ready, setReady] = useState(false);
  const [target, registerTarget] = useState<SummaryTarget | null>(null);
  const [organizing, setOrganizing] = useState(false);
  const [model, setModel] = useState<MemoryModel | null>(null);
  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [stage, setStage] = useState<Stage>(null);
  const [streamText, setStreamText] = useState('');
  const [pinned, setPinnedState] = useState(readPinned);
  const memory = useRef<Promise<void> | null>(null);
  const stopMemory = useRef(false);
  const active = useRef(true);
  const running = useRef(false);
  const reloadSequence = useRef(0);
  const reloadAfterRequest = useRef(false);
  useEffect(() => { active.current = true; return () => { active.current = false; }; }, []);
  useEffect(() => { if (!connected) setOpen(false); }, [connected]);
  const setPinned = (value: boolean) => {
    setPinnedState(value);
    try { localStorage.setItem(PINNED_KEY, value ? '1' : '0'); } catch { /* remembering the choice is optional */ }
  };
  // Progress from the backend: which step the reply is on, and the answer as it is written.
  useEffect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    window.__TAURI__?.event?.listen('niki-chat-progress', event => {
      const payload = event.payload as { stage?: Stage; text?: string | null } | null;
      if (!payload || !active.current) return;
      if (payload.stage) setStage(payload.stage);
      setStreamText(payload.text ?? '');
    }).then(unlisten => { if (cancelled) unlisten(); else stop = unlisten; }).catch(() => undefined);
    return () => { cancelled = true; stop?.(); };
  }, []);
  const reloadPrefs = useCallback(async () => {
    try { const next = await run<Prefs>('codex_prefs'); if (active.current) setPrefs(next); } catch { /* shown as unconsented */ }
  }, [run]);
  useEffect(() => { if (connected) void reloadPrefs(); }, [connected, open, reloadPrefs]);
  const setConsent = async (value: boolean) => {
    const next = await run<Prefs>('codex_set_consent', { consented: value });
    if (active.current) setPrefs(next);
    window.dispatchEvent(new Event(CODEX_PREFS_CHANGED));
    if (value && active.current) setTimeout(() => organize(), 0);
  };
  const reload = useCallback(async () => {
    const sequence = ++reloadSequence.current;
    try {
      const history = await run<ChatMessage[]>('codex_conversation');
      if (active.current && sequence === reloadSequence.current) { setMessages(history); setReady(true); }
    } catch (e) { if (active.current && sequence === reloadSequence.current) { setReady(false); setError(errorMessage(e)); } }
  }, [run]);
  useEffect(() => {
    const update = () => {
      void reloadPrefs();
      if (running.current) { reloadAfterRequest.current = true; return; }
      void reload();
    };
    window.addEventListener(SYNC_COMPLETED, update);
    window.addEventListener(CODEX_PREFS_CHANGED, update);
    return () => {
      window.removeEventListener(SYNC_COMPLETED, update);
      window.removeEventListener(CODEX_PREFS_CHANGED, update);
    };
  }, [reload, reloadPrefs]);
  useEffect(() => {
    if (open) { if (!running.current) void reload(); scheduleSync(0); }
  }, [open, reload, scheduleSync]);
  // Index new diaries and fold old chat in the background; it yields to any message the user sends.
  const organize = useCallback(() => {
    if (memory.current) return;
    stopMemory.current = false;
    setOrganizing(true);
    memory.current = (async () => {
      try {
        let previous = Infinity;
        while (active.current && !stopMemory.current) {
          const { remaining } = await run<{ remaining: number }>('codex_refresh_memory');
          if (remaining === 0 || remaining >= previous) break;
          previous = remaining;
        }
      } catch { /* organizing is best effort; chat still works from recent entries */ }
      finally { memory.current = null; if (active.current) setOrganizing(false); }
    })();
  }, [run]);
  useEffect(() => { if (open && ready && connected) organize(); }, [open, ready, connected, organize]);
  const checkModel = useCallback(async () => {
    try { const next = await run<MemoryModel>('codex_memory_model'); if (active.current) setModel(next); return next; }
    catch { return null; }
  }, [run]);
  useEffect(() => { if (open && connected) void checkModel(); }, [open, connected, checkModel]);
  // The app fetches the search model by itself; while it is missing, follow progress and start
  // indexing as soon as it lands.
  const waitingForModel = open && connected && !!model && !model.installed;
  useEffect(() => {
    if (!waitingForModel) return;
    const timer = setInterval(() => {
      void checkModel().then(next => { if (next?.installed && active.current) organize(); });
    }, 1000);
    return () => clearInterval(timer);
  }, [waitingForModel, checkModel, organize]);

  const request = async (options: { summaryTarget?: SummaryTarget; text?: string } = {}) => {
    const { summaryTarget } = options;
    const text = (options.text ?? draft).trim();
    if (running.current || !connected || !ready || (!summaryTarget && !text)) return;
    ++reloadSequence.current;
    running.current = true; setBusy(true); setError('');
    setStage(summaryTarget ? 'writing' : 'searching'); setStreamText('');
    try {
      if (memory.current) { stopMemory.current = true; await memory.current; }
      let args: Record<string, unknown>;
      if (summaryTarget) {
        const expectedUpdatedAt = await summaryTarget.prepare();
        if (!active.current) return;
        if (!expectedUpdatedAt) throw Error('일기를 저장하지 못해 요약하지 않았습니다. 저장 상태를 확인해 주세요.');
        args = { entryId: summaryTarget.entryId, expectedUpdatedAt };
        setPending(`「${summaryTarget.title || '제목 없는 일기'}」 요약해 줘`);
      } else {
        args = { message: text }; setPending(text);
      }
      const history = await run<ChatMessage[]>(summaryTarget ? 'codex_summarize' : 'codex_chat', args);
      if (active.current) { setMessages(history); if (!summaryTarget && options.text === undefined) setDraft(''); }
      // Keep folding old messages and indexing new diaries between messages.
      if (active.current) setTimeout(organize, 0);
    } catch (e) {
      if (!active.current) return;
      if (errorMessage(e) === 'cancelled') {
        // Stopped by the user: nothing was saved, so the question goes back into the box.
        if (!summaryTarget && options.text === undefined) setDraft(text);
      } else setError(errorMessage(e));
      await reload();
    } finally {
      running.current = false;
      if (reloadAfterRequest.current) { reloadAfterRequest.current = false; void reload(); }
      if (active.current) { setPending(''); setBusy(false); setStage(null); setStreamText(''); }
    }
  };
  const cancel = () => { void run('codex_cancel').catch(() => undefined); };
  const regenerate = async () => {
    if (running.current) return;
    try {
      const retracted = await run<{ text: string; history: ChatMessage[] }>('codex_retract_last', { expectedLastId: messages.at(-1)?.id ?? '' });
      if (!active.current) return;
      setMessages(retracted.history);
      await request({ text: retracted.text });
    } catch (e) { if (active.current) { setError(errorMessage(e)); await reload(); } }
  };
  const clearConversation = async () => {
    if (running.current) return;
    try {
      const history = await run<ChatMessage[]>('codex_clear_conversation');
      if (active.current) { setMessages(history); setError(''); }
    } catch (e) { if (active.current) setError(errorMessage(e)); }
  };
  return <Context.Provider value={{ connected, open, toggle: () => setOpen(value => !value), close: () => setOpen(false),
    messages, organizing, pending, busy, error, ready, stage, streamText, draft, setDraft, reload, target, registerTarget,
    send: text => request({ text }), summarize: () => target ? request({ summaryTarget: target }) : Promise.resolve(),
    cancel, regenerate, clearConversation, model, prefs, setConsent, pinned, setPinned }}>{children}</Context.Provider>;
}
