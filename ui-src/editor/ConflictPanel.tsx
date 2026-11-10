import { TriangleAlert } from 'lucide-react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { timeLabel } from '../lib/format';
import type { Conflict, Entry } from '../types';

const FIELDS: { key: keyof Entry; label: string }[] = [
  { key: 'title', label: '제목' }, { key: 'body', label: '본문' }, { key: 'date', label: '날짜' },
  { key: 'location', label: '장소' }, { key: 'weather', label: '날씨 / 메모' }, { key: 'tags', label: '태그' },
  { key: 'favorite', label: '즐겨찾기' }, { key: 'attachments', label: '첨부' }, { key: 'journal_id', label: '일기장' },
];
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
const live = (entry: Entry | null) => entry && !entry.deleted_at ? entry : null;
export const changedFields = (conflict: Conflict) => {
  const local = live(conflict.local), remote = live(conflict.remote);
  if (!local || !remote) return ['삭제 여부'];
  return FIELDS.filter(({ key }) => !same(local[key], remote[key])).map(item => item.label);
};

function Version({ title, hint, entry, differs }: { title: string; hint: string; entry: Entry | null; differs: string[] }) {
  const shown = live(entry);
  const text = (label: string, value: string) => differs.includes(label) ? <p className="conflict-field"><span>{label}</span>{value || '(비어 있음)'}</p> : null;
  return <div className="conflict-version">
    <h4>{title}</h4>
    <p className="conflict-when">{hint}{shown ? ` · ${timeLabel(shown.updated_at)} 수정` : ''}</p>
    {shown ? <>
      <p className="conflict-title">{shown.title || '제목 없음'}</p>
      {text('장소', shown.location)}{text('날씨 / 메모', shown.weather)}{text('날짜', shown.date)}{text('태그', shown.tags.join(', '))}
      <div className="conflict-body">{shown.body || '(본문 없음)'}</div>
      <p className="conflict-count">{shown.body.length.toLocaleString('ko-KR')}자</p>
    </> : <p className="conflict-body">이 일기는 삭제되었습니다.</p>}
  </div>;
}

export function ConflictPanel({ conflict, compact = false }: { conflict: Conflict; compact?: boolean }) {
  const { run, refresh, action, toast, syncNow } = useApp();
  const differs = changedFields(conflict);
  const resolve = (choice: 'local' | 'remote') => void action(async () => {
    await run('resolve_sync_conflict', { conflictId: conflict.id, choice });
    await refresh(); toast('충돌을 해결했습니다. 동기화합니다'); void syncNow(false);
  });
  return <section className="conflict-panel" data-conflict-id={conflict.id} role="alert">
    <div className="conflict-head"><TriangleAlert size={16} />
      <div><strong>{compact ? (live(conflict.local)?.title || live(conflict.remote)?.title || '제목 없음') : '동기화 충돌'}</strong>
        <p>두 기기에서 같은 일기가 다르게 수정되었습니다. 남길 버전을 선택하세요. 다른 버전은 사라집니다.{differs.length > 0 && <> 다른 부분: <b>{differs.join(', ')}</b></>}</p></div></div>
    <div className="conflict-grid">
      <Version title="이 기기" hint="이 컴퓨터의 버전" entry={conflict.local} differs={differs} />
      <Version title="다른 기기" hint="동기화된 최신 버전" entry={conflict.remote} differs={differs} />
    </div>
    <div className="conflict-actions">
      <Button variant="outline" size="sm" data-action="resolve-conflict" onClick={() => resolve('local')}>이 기기 버전 유지</Button>
      <Button size="sm" data-action="resolve-conflict" onClick={() => resolve('remote')}>다른 기기 버전 사용</Button>
    </div>
  </section>;
}
