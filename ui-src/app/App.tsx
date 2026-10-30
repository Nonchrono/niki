import { useState } from 'react';
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom';
import { AppProvider, useApp } from './AppContext';
import { AppShell } from './AppShell';
import { TimelinePage } from '../pages/TimelinePage';
import { CalendarPage } from '../pages/CalendarPage';
import { MediaPage } from '../pages/MediaPage';
import { MemoriesPage } from '../pages/MemoriesPage';
import { TrashPage } from '../pages/TrashPage';
import { SettingsPage } from '../pages/SettingsPage';

function AppRoutes() {
  const { data, run, refresh, action, scheduleSync } = useApp();
  const [password, setPassword] = useState('');
  if (!window.__TAURI__?.core) return <div className="empty"><h2>데스크톱 앱에서 열어 주세요</h2></div>;
  if (!data) return <div className="empty">불러오는 중…</div>;
  if (data.locked) return <div className="lock-page"><div className="lock-card"><img className="brand-mark" src="./icon.svg" alt="" style={{ margin: 'auto' }} /><h2>일기를 다시 열어 주세요</h2><p>잠금 암호를 입력하세요.</p><input id="unlock-password" type="password" placeholder="암호" autoFocus value={password} onChange={event => setPassword(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') document.getElementById('unlock-button')?.click(); }} /><button id="unlock-button" className="primary" data-action="unlock" onClick={() => void action(async () => { await run('unlock', { password }); await refresh(); scheduleSync(0); })}>잠금 해제</button></div></div>;
  return <Routes>
    <Route element={<AppShell />}>
      <Route index element={<Navigate to="/all" replace />} />
      <Route path="/all" element={<TimelinePage />} />
      <Route path="/all/entries/:entryId" element={<TimelinePage />} />
      <Route path="/journals/:journalId" element={<TimelinePage />} />
      <Route path="/journals/:journalId/entries/:entryId" element={<TimelinePage />} />
      <Route path="/calendar" element={<CalendarPage />} />
      <Route path="/media" element={<MediaPage />} />
      <Route path="/memories" element={<MemoriesPage />} />
      <Route path="/trash" element={<TrashPage />} />
      <Route path="/settings/:tab" element={<SettingsPage />} />
      <Route path="/settings" element={<Navigate to="/settings/sync" replace />} />
      <Route path="*" element={<Navigate to="/all" replace />} />
    </Route>
  </Routes>;
}
export function App() { return <HashRouter><AppProvider><AppRoutes /></AppProvider></HashRouter>; }
