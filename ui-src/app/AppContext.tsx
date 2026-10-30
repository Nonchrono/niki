import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke, setMutationListener } from '../lib/tauri';
import { errorMessage } from '../lib/format';
import type { Snapshot, UpdateInfo } from '../types';

type LeaveHandler = () => Promise<boolean>;
interface AppState {
  data: Snapshot | null; update: UpdateInfo | null; setUpdate: (value: UpdateInfo | null) => void;
  refresh: () => Promise<Snapshot>; run: <T>(name: string, args?: Record<string, unknown>) => Promise<T>;
  action: (work: () => Promise<void>) => Promise<void>; toast: (message: string, bad?: boolean) => void;
  go: (path: string) => Promise<boolean>; registerLeave: (handler: LeaveHandler | null) => void;
  saveBeforeLeaving: () => Promise<void>;
  syncNow: (manual?: boolean) => Promise<void>; scheduleSync: (delay?: number) => void;
  syncing: boolean; media: (id: string) => Promise<string>; clearMedia: (id?: string) => void;
}
const Context = createContext<AppState | null>(null);
export const useApp = () => { const context = useContext(Context); if (!context) throw Error('Missing AppProvider'); return context; };

export function AppProvider({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const [data, setData] = useState<Snapshot | null>(null);
  const dataRef = useRef<Snapshot | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [notice, setNotice] = useState<{ message: string; bad: boolean } | null>(null);
  const noticeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const leaveHandler = useRef<LeaveHandler | null>(null);
  const mediaCache = useRef(new Map<string, string>());
  const syncTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const syncRunning = useRef(false);
  const syncQueued = useRef(false);
  const retryDelay = useRef(15_000);
  const [syncing, setSyncing] = useState(false);
  const scheduleRef = useRef<(delay?: number) => void>(() => {});

  const toast = useCallback((message: string, bad = false) => {
    setNotice({ message, bad });
    if (noticeTimer.current) clearTimeout(noticeTimer.current);
    noticeTimer.current = setTimeout(() => setNotice(null), 4200);
  }, []);
  const refresh = useCallback(async () => {
    const next = await invoke<Snapshot>('bootstrap');
    dataRef.current = next;
    setData(next);
    if (next.locked) mediaCache.current.clear();
    return next;
  }, []);
  const run = useCallback(<T,>(name: string, args: Record<string, unknown> = {}) => invoke<T>(name, args), []);
  const action = useCallback(async (work: () => Promise<void>) => { try { await work(); } catch (error) { console.error(error); toast(errorMessage(error), true); } }, [toast]);
  const registerLeave = useCallback((handler: LeaveHandler | null) => { leaveHandler.current = handler; }, []);
  const leave = useCallback(async () => {
    if (leaveHandler.current && !await leaveHandler.current()) throw new Error('저장하지 못했습니다. 다시 시도해 주세요.');
  }, []);
  const go = useCallback(async (path: string) => {
    try { await leave(); navigate(path); return true; }
    catch (error) { toast(errorMessage(error), true); return false; }
  }, [leave, navigate, toast]);
  const media = useCallback(async (id: string) => {
    if (dataRef.current?.locked) throw Error('앱이 잠겨 있습니다');
    const cached = mediaCache.current.get(id);
    if (cached) return cached;
    const item = await invoke<{ mime: string; data: string }>('get_attachment', { attachmentId: id });
    if (dataRef.current?.locked) throw Error('앱이 잠겨 있습니다');
    if (!/^(image\/(png|jpeg|gif|webp)|video\/(mp4|quicktime)|audio\/(mp4|mpeg|wav)|application\/(pdf|octet-stream))$/.test(item.mime)) throw Error('허용되지 않은 미디어 유형');
    const url = `data:${item.mime};base64,${item.data}`;
    mediaCache.current.set(id, url);
    return url;
  }, []);
  const clearMedia = useCallback((id?: string) => { if (id) mediaCache.current.delete(id); else mediaCache.current.clear(); }, []);
  const syncNow = useCallback(async (manual = false) => {
    const status = dataRef.current?.sync_status;
    if (dataRef.current?.locked || !status?.connected || status.sync_paused) return;
    if (syncRunning.current) { syncQueued.current = true; return; }
    if (syncTimer.current) clearTimeout(syncTimer.current);
    syncRunning.current = true; syncQueued.current = false; setSyncing(true);
    let failed = false;
    try {
      await leave();
      const result = await invoke<{ warnings?: string[] }>('drive_sync_now');
      await refresh();
      retryDelay.current = 15_000;
      if (result.warnings?.length) toast(result.warnings[0], true);
      else if (manual) toast('동기화가 완료되었습니다');
    } catch (error) {
      failed = true;
      try { await refresh(); } catch (refreshError) { console.warn(refreshError); }
      if (manual) throw error;
      console.warn('Auto sync failed:', error);
    } finally {
      syncRunning.current = false; setSyncing(false);
      if (dataRef.current?.sync_status?.connected && !dataRef.current.sync_status.sync_paused && (failed || syncQueued.current)) {
        scheduleRef.current(failed ? retryDelay.current : 8000);
        if (failed) retryDelay.current = Math.min(retryDelay.current * 2, 300_000);
      }
    }
  }, [leave, refresh, toast]);
  const scheduleSync = useCallback((delay = 8000) => {
    if (dataRef.current?.locked || !dataRef.current?.sync_status?.connected || dataRef.current.sync_status.sync_paused) return;
    if (syncRunning.current) { syncQueued.current = true; return; }
    if (syncTimer.current) clearTimeout(syncTimer.current);
    syncTimer.current = setTimeout(() => { syncTimer.current = null; void syncNow(); }, delay);
  }, [syncNow]);
  scheduleRef.current = scheduleSync;

  useEffect(() => { setMutationListener(() => scheduleRef.current()); return () => setMutationListener(null); }, []);
  useEffect(() => {
    if (!window.__TAURI__?.core) return;
    let unlisten: (() => void) | undefined;
    void refresh().then(next => {
      if (!next.locked) scheduleRef.current(0);
      const cfg = next.updater_config;
      if (!next.locked && cfg?.auto_check && cfg.endpoint && !cfg.endpoint.includes('YOUR_GITHUB_ID')) {
        void invoke<UpdateInfo>('updater_check').then(info => {
          if (info.update_available && info.latest_version !== cfg.skipped_version) setUpdate(info);
        }).catch(error => console.warn('Auto update check skipped:', error));
      }
    }).catch(error => toast(errorMessage(error), true));
    window.__TAURI__.event?.listen('niki-sync-finished', () => { if (!syncRunning.current) void refresh().catch(console.warn); }).then(stop => { unlisten = stop; }).catch(console.warn);
    const online = () => scheduleRef.current(1000);
    window.addEventListener('online', online);
    const interval = setInterval(() => scheduleRef.current(0), 300_000);
    return () => { unlisten?.(); window.removeEventListener('online', online); clearInterval(interval); if (syncTimer.current) clearTimeout(syncTimer.current); };
  }, [refresh, toast]);

  return <Context.Provider value={{ data, update, setUpdate, refresh, run, action, toast, go, registerLeave, saveBeforeLeaving: leave, syncNow, scheduleSync, syncing, media, clearMedia }}>
    {children}<div id="toast" role="status" aria-live="polite" className={notice ? `show${notice.bad ? ' error' : ''}` : ''}>{notice?.message}</div>
  </Context.Provider>;
}
