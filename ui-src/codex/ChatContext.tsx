import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { useApp } from '../app/AppContext';
import { errorMessage } from '../lib/format';
import { useGptConnection } from './useGptConnection';

export type ChatMessage = { id: string; role: 'user' | 'assistant'; content: string; kind: 'chat' | 'summary' };
type SummaryTarget = { entryId: string; title: string; disabled: boolean; prepare: () => Promise<string | null> };
type ChatState = {
  connected: boolean; open: boolean; toggle: () => void; close: () => void;
  messages: ChatMessage[]; organizing: boolean; pending: string; busy: boolean; error: string; ready: boolean;
  draft: string; setDraft: (value: string) => void; reload: () => Promise<void>;
  target: SummaryTarget | null; registerTarget: (target: SummaryTarget | null) => void;
  send: () => Promise<void>; summarize: () => Promise<void>;
};
const Context = createContext<ChatState | null>(null);
export const useChat = () => { const value = useContext(Context); if (!value) throw Error('Missing ChatProvider'); return value; };

export function ChatProvider({ children }: { children: ReactNode }) {
  const { run } = useApp();
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
  const memory = useRef<Promise<void> | null>(null);
  const stopMemory = useRef(false);
  const active = useRef(true);
  const running = useRef(false);
  useEffect(() => { active.current = true; return () => { active.current = false; }; }, []);
  useEffect(() => { if (!connected) setOpen(false); }, [connected]);
  const reload = useCallback(async () => {
    try {
      const history = await run<ChatMessage[]>('codex_conversation');
      if (active.current) { setMessages(history); setReady(true); }
    } catch (e) { if (active.current) { setReady(false); setError(errorMessage(e)); } }
  }, [run]);
  useEffect(() => { if (open && !ready) void reload(); }, [open, reload]);
  // Summarize older diaries in the background so Niki can recall them; it yields to any message the user sends.
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
  const request = async (summaryTarget?: SummaryTarget) => {
    const text = draft.trim();
    if (running.current || !connected || !ready || (!summaryTarget && !text)) return;
    running.current = true; setBusy(true); setError('');
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
      if (active.current) { setMessages(history); if (!summaryTarget) setDraft(''); }
      // Keep folding old messages and summarizing new diaries between messages.
      if (active.current) setTimeout(organize, 0);
    } catch (e) {
      if (active.current) { setError(errorMessage(e)); await reload(); }
    } finally {
      running.current = false;
      if (active.current) { setPending(''); setBusy(false); }
    }
  };
  return <Context.Provider value={{ connected, open, toggle: () => setOpen(value => !value), close: () => setOpen(false),
    messages, organizing, pending, busy, error, ready, draft, setDraft, reload, target, registerTarget,
    send: () => request(), summarize: () => target ? request(target) : Promise.resolve() }}>{children}</Context.Provider>;
}
