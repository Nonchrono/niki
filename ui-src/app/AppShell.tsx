import { useState } from 'react';
import { Outlet, useLocation } from 'react-router-dom';
import {
  BookOpenText, CalendarDays, ChevronsLeft, ChevronsRight, Images,
  Pencil, Plus, RefreshCw, Settings2, Sparkles, Trash2, type LucideIcon,
} from 'lucide-react';
import { Button } from '../components/ui/button';
import { useApp } from './AppContext';
import { accountLabel, localDate } from '../lib/format';
import type { Journal } from '../types';

const sections: { path: string; action: string; Icon: LucideIcon; label: string }[] = [
  { path: '/all', action: 'all', Icon: BookOpenText, label: '전체 기록' },
  { path: '/calendar', action: 'calendar', Icon: CalendarDays, label: '달력' },
  { path: '/media', action: 'media', Icon: Images, label: '미디어' },
  { path: '/memories', action: 'memories', Icon: Sparkles, label: '이 날의 기록' },
];

export function AppShell() {
  const { data, update, setUpdate, go, run, refresh, action, toast, syncing, syncNow, saveBeforeLeaving } = useApp();
  const location = useLocation();
  const journalId = /^\/journals\/([^/]+)/.exec(location.pathname)?.[1]
    || new URLSearchParams(location.search).get('journal') || undefined;
  const [collapsed, setCollapsed] = useState(false);
  const [journalMenu, setJournalMenu] = useState<{ journal: Journal; x: number; y: number } | null>(null);
  if (!data) return <div className="empty">불러오는 중…</div>;

  const active = data.entries.filter(entry => !entry.deleted_at);
  const timeline = location.pathname.startsWith('/all') || location.pathname.startsWith('/journals/');
  const title = timeline ? '' : location.pathname.startsWith('/settings') ? '설정'
    : location.pathname.startsWith('/calendar') ? '달력'
    : location.pathname.startsWith('/media') ? '미디어'
    : location.pathname.startsWith('/memories') ? '이 날의 기록' : '휴지통';
  const navigateSection = (path: string) => go(journalId && ['/calendar', '/media', '/memories'].includes(path)
    ? `${path}?journal=${encodeURIComponent(journalId)}` : path);
  const side = (path: string, actionName: string, Icon: LucideIcon, label: string, count?: number) => (
    <button key={path} className={`side-row ${location.pathname.startsWith(path) ? 'active' : ''}`}
      data-action={actionName} title={label} onClick={() => void navigateSection(path)}>
      <span className="icon"><Icon size={18} strokeWidth={1.8} /></span>
      <span className="side-label">{label}</span><span className="count">{count ?? ''}</span>
    </button>
  );
  const createEntry = () => action(async () => {
    await saveBeforeLeaving();
    const id = journalId || data.journals[0]?.id;
    if (!id) throw Error('일기장이 없습니다');
    const entryId = await run<string>('save_entry', {
      input: { journalId: id, title: '', body: '', date: localDate(), favorite: false, tags: [], location: '', weather: '' },
    });
    await refresh();
    await go(`/journals/${id}/entries/${entryId}`);
    setTimeout(() => document.getElementById('entry-title')?.focus(), 0);
  });
  const installUpdate = () => action(async () => {
    toast('새 버전 다운로드 및 검증 중…');
    const version = await run<string>('updater_install');
    toast(`v${version} 업데이트 완료! 앱을 재시작합니다…`);
  });

  return <div className={`shell${collapsed ? ' collapsed' : ''}`}>
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-text" title={data.nickname || '안녕하세요!'}>{data.nickname?.trim() || '안녕하세요!'}</span>
        <Button variant="outline" size="icon-sm" className="sidebar-toggle"
          title={collapsed ? '메뉴바 열기' : '메뉴바 접기'} aria-label={collapsed ? '메뉴바 열기' : '메뉴바 접기'}
          onClick={() => setCollapsed(!collapsed)}>
          {collapsed ? <ChevronsRight /> : <ChevronsLeft />}
        </Button>
      </div>
      <div className="side-caption">둘러보기</div>
      {sections.map(item => side(item.path, item.action, item.Icon, item.label, item.action === 'all' ? active.length : undefined))}
      <div className="side-caption">일기장 목록</div>
      {data.journals.map(journal => <button key={journal.id}
        className={`side-row ${journalId === journal.id ? 'active' : ''}`} data-action="journal" data-id={journal.id}
        title={`${journal.name} (우클릭하여 수정·삭제)`} onClick={() => void go(`/journals/${journal.id}`)}
        onContextMenu={event => {
          event.preventDefault();
          setJournalMenu({ journal, x: Math.max(8, Math.min(event.clientX, window.innerWidth - 190)), y: Math.max(8, Math.min(event.clientY, window.innerHeight - 120)) });
        }}>
        <span className="dot" style={{ background: journal.color }} />
        <span className="side-label">{journal.name}</span>
        <span className="count">{active.filter(entry => entry.journal_id === journal.id).length}</span>
      </button>)}
      <button className="side-row side-add" data-action="new-journal" onClick={() => void action(async () => {
        await saveBeforeLeaving();
        const name = prompt('새 일기장 이름');
        if (!name?.trim()) return;
        const id = await run<string>('save_journal', { id: null, name, color: '#576b61' });
        await refresh();
        await go(`/journals/${id}`);
      })}><span className="icon"><Plus size={18} /></span><span className="side-label">새 일기장</span></button>
      <div className="side-bottom">
        {side('/trash', 'trash', Trash2, '휴지통', data.entries.length - active.length)}
        {side('/settings', 'settings', Settings2, '설정', data.sync_conflicts.length || undefined)}
      </div>
    </aside>
    <main className="workspace">
      <header className="topbar"><div className="top-title">{title}</div><div className="top-spacer" />
        {data.sync_status.connected && <Button variant="outline" size="sm" data-action="drive-sync"
          title={accountLabel(data.sync_status)} disabled={syncing} onClick={() => void action(() => syncNow(true))}>
          <RefreshCw className={syncing ? 'animate-spin' : ''} />
          {syncing ? '동기화 중' : `동기화${data.sync_status.pending_upload_count ? ` (${data.sync_status.pending_upload_count})` : ''}`}
        </Button>}
        <Button size="sm" data-action="new-entry" onClick={() => void createEntry()}><Plus />새 기록</Button>
      </header>
      {update?.update_available && update.latest_version !== data.updater_config?.skipped_version &&
        <div className="update-banner"><strong>새 버전(v{update.latest_version}) 업데이트가 준비되었습니다.</strong>
          <span className="update-banner-notes">{update.notes}</span>
          <div className="update-banner-actions">
            <Button size="sm" onClick={() => void installUpdate()}>지금 다운로드 및 재시작</Button>
            <Button size="sm" variant="outline" onClick={() => setUpdate(null)}>나중에</Button>
            <Button size="sm" variant="outline" onClick={() => void action(async () => {
              await run('updater_skip_version', { version: update.latest_version });
              setUpdate(null);
              await refresh();
            })}>이 버전 건너뛰기</Button>
          </div>
        </div>}
      <div className="content" id="content"><Outlet /></div>
    </main>
    {journalMenu && <div className="menu-backdrop" onClick={() => setJournalMenu(null)}>
      <div className="context-menu journal-menu" style={{ left: journalMenu.x, top: journalMenu.y }} onClick={event => event.stopPropagation()}>
        <div className="context-menu-head"><span className="dot" style={{ background: journalMenu.journal.color }} />{journalMenu.journal.name}</div>
        <button className="context-menu-item" data-action="edit-journal" onClick={() => void action(async () => {
          const name = prompt('일기장 이름', journalMenu.journal.name);
          if (!name?.trim()) return;
          const color = prompt('색상 코드 (#RRGGBB)', journalMenu.journal.color);
          if (!color) return;
          await run('save_journal', { id: journalMenu.journal.id, name, color });
          await refresh();
          setJournalMenu(null);
        })}><Pencil size={15} />수정</button>
        <button className="context-menu-item danger" data-action="delete-journal" onClick={() => void action(async () => {
          if (!confirm('빈 일기장을 삭제할까요?')) return;
          await run('delete_journal', { id: journalMenu.journal.id });
          await refresh();
          await go('/all');
          setJournalMenu(null);
        })}><Trash2 size={15} />삭제</button>
      </div>
    </div>}
  </div>;
}
