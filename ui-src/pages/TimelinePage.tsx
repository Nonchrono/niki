import { useState } from 'react';
import { useSearchParams, useParams } from 'react-router-dom';
import { Cloud, Heart, Image, PenLine, Plus, Search, TriangleAlert } from 'lucide-react';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { NativeSelect, NativeSelectOption } from '../components/ui/native-select';
import { useApp } from '../app/AppContext';
import { EntryEditor } from '../editor/EntryEditor';
import { plainText } from '../editor/markdown';
import { dateLabel } from '../lib/format';

export function TimelinePage() {
  const { data, go } = useApp();
  const { journalId, entryId } = useParams();
  const [query, setQuery] = useState('');
  const [tag, setTag] = useState('');
  const [favorites, setFavorites] = useState(false);
  const [params, setParams] = useSearchParams();
  if (!data) return null;

  const date = params.get('date') || '';
  const active = data.entries.filter(entry => !entry.deleted_at);
  const name = data.journals.find(journal => journal.id === journalId)?.name || '모든 기록';
  const search = query.trim().toLocaleLowerCase();
  const entries = active.filter(entry =>
    (!journalId || entry.journal_id === journalId)
    && (!date || entry.date === date)
    && (!favorites || entry.favorite)
    && (!tag || entry.tags.includes(tag))
    && (!search || [entry.title, entry.body, entry.location, entry.weather, ...entry.tags]
      .join(' ').toLocaleLowerCase().includes(search))
  ).sort((a, b) => b.date.localeCompare(a.date) || b.updated_at.localeCompare(a.updated_at));
  const tags = [...new Set(active.flatMap(entry => entry.tags))].sort();
  const selected = data.entries.find(entry => entry.id === entryId && !entry.deleted_at);
  const base = journalId ? `/journals/${journalId}` : '/all';

  return <>
    <section className="list-pane">
      <div className="list-head">
        <h2>{date ? dateLabel(date) : name}</h2>
        <div className="subtle">{entries.length}개의 기록 {date && <button className="text-link" data-action="clear-date" onClick={() => setParams({})}>날짜 필터 지우기</button>}</div>
        <label className="search-wrap list-search"><span><Search size={15} /></span>
          <Input id="search" className="search" placeholder="기록, 태그, 장소 검색" value={query} onChange={event => setQuery(event.target.value)} />
        </label>
      </div>
      <div className="list-controls">
        <Button variant="outline" size="xs" className={`chip ${favorites ? 'active' : ''}`} data-action="favorites" onClick={() => setFavorites(!favorites)}>
          <Heart size={13} fill={favorites ? 'currentColor' : 'none'} />즐겨찾기
        </Button>
        <NativeSelect size="sm" id="tag-filter" className="filter-select" value={tag} onChange={event => setTag(event.target.value)}>
          <NativeSelectOption value="">모든 태그</NativeSelectOption>
          {tags.map(value => <NativeSelectOption key={value} value={value}># {value}</NativeSelectOption>)}
        </NativeSelect>
      </div>
      <div className="entry-list">
        {entries.length ? entries.map(entry => <button key={entry.id}
          className={`entry-card ${entryId === entry.id ? 'active' : ''}`}
          data-action="entry" data-id={entry.id} onClick={() => void go(`${base}/entries/${entry.id}`)}>
          <div className="entry-date">{dateLabel(entry.date)}{!journalId && ` · ${data.journals.find(journal => journal.id === entry.journal_id)?.name || '일기장'}`}</div>
          <h3>{entry.title || '제목 없는 기록'}</h3>
          <p>{plainText(entry.body.slice(0, 1200)) || '내용을 적어 보세요'}</p>
          <div className="entry-meta">
            {entry.favorite && <span title="즐겨찾기"><Heart size={11} fill="currentColor" /></span>}
            {entry.attachments.length > 0 && <span><Image size={11} />{entry.attachments.length}</span>}
            {data.sync_conflicts.some(item => item.entry_id === entry.id)
              ? <span title="동기화 충돌"><TriangleAlert size={11} />충돌</span>
              : data.synced_entry_ids.includes(entry.id) && <span title="동기화됨"><Cloud size={11} /></span>}
            {entry.tags.slice(0, 2).map(value => <span key={value}>#{value}</span>)}
          </div>
        </button>) : <div className="empty"><p>조건에 맞는 기록이 없습니다.</p></div>}
      </div>
    </section>
    <section className="editor-pane" id="editor-pane">
      {selected ? <EntryEditor key={selected.id} entry={selected} />
        : <div className="empty" style={{ height: '100%' }}>
          <div className="empty-art"><PenLine size={32} /></div>
          <h2>오늘을 기록해 보세요</h2>
          <p>작은 순간도 시간이 지나면 소중한 이야기가 됩니다.</p>
          <Button size="sm" data-action="new-entry" onClick={() => document.querySelector<HTMLButtonElement>('.topbar [data-action="new-entry"]')?.click()}>
            <Plus />새 기록 쓰기
          </Button>
        </div>}
    </section>
  </>;
}
