import { useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { Button } from '../components/ui/button';
import { useApp } from '../app/AppContext';
import { localDate } from '../lib/format';

export function CalendarPage() {
  const { data, go } = useApp();
  const [params] = useSearchParams();
  const journalId = params.get('journal');
  const [month, setMonth] = useState(() => new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  if (!data) return null;
  const year = month.getFullYear(), index = month.getMonth(), offset = new Date(year, index, 1).getDay();

  return <div className="wide-view">
    <div className="wide-head"><h2>{year}년 {index + 1}월</h2><span className="spacer" />
      <Button variant="outline" size="icon-sm" data-action="prev-month" aria-label="이전 달" onClick={() => setMonth(new Date(year, index - 1, 1))}><ChevronLeft /></Button>
      <Button variant="outline" size="sm" data-action="today-month" onClick={() => setMonth(new Date(new Date().getFullYear(), new Date().getMonth(), 1))}>오늘</Button>
      <Button variant="outline" size="icon-sm" data-action="next-month" aria-label="다음 달" onClick={() => setMonth(new Date(year, index + 1, 1))}><ChevronRight /></Button>
    </div>
    <div className="calendar">
      {['일', '월', '화', '수', '목', '금', '토'].map(day => <div key={day} className="weekday">{day}</div>)}
      {Array.from({ length: 42 }, (_, i) => {
        const date = new Date(year, index, i - offset + 1);
        const key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
        const entries = data.entries.filter(entry => !entry.deleted_at && (!journalId || entry.journal_id === journalId) && entry.date === key);
        return <button key={i} className={`day ${date.getMonth() === index ? '' : 'outside'} ${key === localDate() ? 'today' : ''}`}
          data-action="calendar-day" onClick={() => void go(`${journalId ? `/journals/${journalId}` : '/all'}?date=${key}`)}>
          <span>{date.getDate()}</span>
          {entries.slice(0, 3).map(entry => <em key={entry.id}>● {entry.title || '제목 없는 기록'}</em>)}
        </button>;
      })}
    </div>
  </div>;
}
