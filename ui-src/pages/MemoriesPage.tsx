import { useApp } from '../app/AppContext';
import { useSearchParams } from 'react-router-dom';
import { plainText } from '../editor/markdown';
import { dateLabel, localDate } from '../lib/format';

export function MemoriesPage() {
  const { data, go } = useApp(); if (!data) return null;
  const [params] = useSearchParams(); const journalId = params.get('journal');
  const now = new Date(), suffix = `${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`;
  const items = data.entries.filter(entry => !entry.deleted_at && (!journalId || entry.journal_id === journalId) && entry.date.endsWith(suffix) && entry.date !== localDate());
  return <div className="wide-view"><div className="wide-head"><h2>이 날의 기록</h2></div>{items.length ? items.map(entry => <div key={entry.id} className="panel-card"><span className="subtle">{dateLabel(entry.date)}</span><h3>{entry.title || '제목 없는 기록'}</h3><p>{plainText(entry.body.slice(0, 4000)).slice(0, 350)}</p><button className="ghost" data-action="entry" onClick={() => void go(`/journals/${entry.journal_id}/entries/${entry.id}`)}>열기</button></div>) : <div className="empty"><div className="empty-art">✧</div><h2>지난 기록이 없습니다</h2><p>오늘을 기록해 두면 내년에 다시 만날 수 있습니다.</p></div>}</div>;
}
