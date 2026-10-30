import { useState } from 'react';
import { Outlet, useLocation } from 'react-router-dom';
import { useApp } from './AppContext';
import { accountLabel, localDate } from '../lib/format';
import type { Journal } from '../types';

const sections = [
  { path: '/all', action: 'all', icon: '▤', label: '전체 기록' },
  { path: '/calendar', action: 'calendar', icon: '▦', label: '달력' },
  { path: '/media', action: 'media', icon: '▧', label: '미디어' },
  { path: '/memories', action: 'memories', icon: '✧', label: '이 날의 기록' },
];
export function AppShell() {
  const { data, update, setUpdate, go, run, refresh, action, toast, syncing, syncNow, saveBeforeLeaving } = useApp();
  const location = useLocation();
  const journalId = /^\/journals\/([^/]+)/.exec(location.pathname)?.[1] || new URLSearchParams(location.search).get('journal') || undefined;
  const [collapsed, setCollapsed] = useState(false);
  const [journalMenu, setJournalMenu] = useState<{ journal: Journal; x: number; y: number } | null>(null);
  if (!data) return <div className="empty">불러오는 중…</div>;
  const active = data.entries.filter(entry => !entry.deleted_at);
  const timeline = location.pathname.startsWith('/all') || location.pathname.startsWith('/journals/');
  const title = timeline ? '' : location.pathname.startsWith('/settings') ? '설정' : location.pathname.startsWith('/calendar') ? '달력' : location.pathname.startsWith('/media') ? '미디어' : location.pathname.startsWith('/memories') ? '이 날의 기록' : '휴지통';
  const side = (path: string, actionName: string, icon: string, label: string, count?: number) => <button key={path} className={`side-row ${location.pathname.startsWith(path) ? 'active' : ''}`} data-action={actionName} title={label} onClick={() => void go(journalId && ['/calendar', '/media', '/memories'].includes(path) ? `${path}?journal=${encodeURIComponent(journalId)}` : path)}><span className="icon">{icon}</span><span className="side-label">{label}</span><span className="count">{count ?? ''}</span></button>;
  const createEntry = () => action(async () => {
    await saveBeforeLeaving();
    const id = journalId || data.journals[0]?.id;
    if (!id) throw Error('일기장이 없습니다');
    const entryId = await run<string>('save_entry', { input: { journalId: id, title: '', body: '', date: localDate(), favorite: false, tags: [], location: '', weather: '' } });
    await refresh();
    await go(`/journals/${id}/entries/${entryId}`);
    setTimeout(() => document.getElementById('entry-title')?.focus(), 0);
  });
  const installUpdate = () => action(async () => { toast('새 버전 다운로드 및 검증 중…'); const version = await run<string>('updater_install'); toast(`v${version} 업데이트 완료! 앱을 재시작합니다…`); });

  return <div className={`shell${collapsed ? ' collapsed' : ''}`}>
    <aside className="sidebar">
      <div className="brand"><span className="brand-text" title={data.nickname || '안녕하세요!'}>{data.nickname?.trim() || '안녕하세요!'}</span><button className="sidebar-toggle" title={collapsed ? '메뉴바 열기' : '메뉴바 접기'} aria-label={collapsed ? '메뉴바 열기' : '메뉴바 접기'} onClick={() => setCollapsed(!collapsed)}>{collapsed ? '»' : '«'}</button></div>
      <div className="side-caption">둘러보기</div>
      {sections.map(item => side(item.path, item.action, item.icon, item.label, item.action === 'all' ? active.length : undefined))}
      <div className="side-caption">일기장 목록</div>
      {data.journals.map(j => <button key={j.id} className={`side-row ${journalId === j.id ? 'active' : ''}`} data-action="journal" data-id={j.id} title={`${j.name} (우클릭하여 수정·삭제)`} onClick={() => void go(`/journals/${j.id}`)} onContextMenu={event => { event.preventDefault(); setJournalMenu({ journal: j, x: Math.max(8, Math.min(event.clientX, window.innerWidth - 190)), y: Math.max(8, Math.min(event.clientY, window.innerHeight - 120)) }); }}><span className="dot" style={{ background: j.color }} /><span className="side-label">{j.name}</span><span className="count">{active.filter(e => e.journal_id === j.id).length}</span></button>)}
      <button className="side-row side-add" data-action="new-journal" onClick={() => void action(async () => { await saveBeforeLeaving(); const name = prompt('새 일기장 이름'); if (!name?.trim()) return; const id = await run<string>('save_journal', { id: null, name, color: '#576b61' }); await refresh(); await go(`/journals/${id}`); })}><span className="icon">＋</span><span className="side-label">새 일기장</span></button>
      <div className="side-bottom">{side('/trash', 'trash', '♜', '휴지통', data.entries.length - active.length)}{side('/settings', 'settings', '⚙', '설정', data.sync_conflicts.length || undefined)}</div>
    </aside>
    <main className="workspace"><header className="topbar"><div className="top-title">{title}</div><div className="top-spacer" />{data.sync_status.connected && <button className="ghost" data-action="drive-sync" title={accountLabel(data.sync_status)} disabled={syncing} onClick={() => void action(() => syncNow(true))}>{syncing ? '↻ 동기화 중' : `↻ 동기화${data.sync_status.pending_upload_count ? ` (${data.sync_status.pending_upload_count})` : ''}`}</button>}<button className="primary" data-action="new-entry" onClick={() => void createEntry()}>＋ 새 기록</button></header>
      {update?.update_available && update.latest_version !== data.updater_config?.skipped_version && <div className="update-banner"><strong>새 버전(v{update.latest_version}) 업데이트가 준비되었습니다.</strong><span className="update-banner-notes">{update.notes}</span><div className="update-banner-actions"><button className="primary" onClick={() => void installUpdate()}>지금 다운로드 및 재시작</button><button className="ghost" onClick={() => setUpdate(null)}>나중에</button><button className="ghost" onClick={() => void action(async () => { await run('updater_skip_version', { version: update.latest_version }); setUpdate(null); await refresh(); })}>이 버전 건너뛰기</button></div></div>}
      <div className="content" id="content"><Outlet /></div>
    </main>
    {journalMenu && <div className="menu-backdrop" onClick={() => setJournalMenu(null)}><div className="context-menu journal-menu" style={{ left: journalMenu.x, top: journalMenu.y }} onClick={event => event.stopPropagation()}><div className="context-menu-head"><span className="dot" style={{ background: journalMenu.journal.color }} />{journalMenu.journal.name}</div><button className="context-menu-item" data-action="edit-journal" onClick={() => void action(async () => { const name = prompt('일기장 이름', journalMenu.journal.name); if (!name?.trim()) return; const color = prompt('색상 코드 (#RRGGBB)', journalMenu.journal.color); if (!color) return; await run('save_journal', { id: journalMenu.journal.id, name, color }); await refresh(); setJournalMenu(null); })}>✎ 수정</button><button className="context-menu-item danger" data-action="delete-journal" onClick={() => void action(async () => { if (!confirm('빈 일기장을 삭제할까요?')) return; await run('delete_journal', { id: journalMenu.journal.id }); await refresh(); await go('/all'); setJournalMenu(null); })}>× 삭제</button></div></div>}
  </div>;
}
