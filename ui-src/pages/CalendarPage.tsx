import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { Button } from '../components/ui/button';
import { useApp } from '../app/AppContext';
import { dateLabel, localDate } from '../lib/format';
import { getDayColorType, getHoliday, useHolidays } from '../lib/holidays';

export function CalendarPage() {
  const { data, go } = useApp();
  const [params] = useSearchParams();
  const journalId = params.get('journal');
  const [month, setMonth] = useState(() => new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  const [expandedDays, setExpandedDays] = useState<Set<string>>(() => new Set());
  const year = month.getFullYear(), index = month.getMonth(), offset = new Date(year, index, 1).getDay();
  const holidays = useHolidays(year);

  useEffect(() => setExpandedDays(new Set()), [year, index, journalId]);

  if (!data) return null;

  return <div className="wide-view">
    <div className="wide-head"><h2>{year}년 {index + 1}월</h2><span className="spacer" />
      <Button variant="outline" size="icon-sm" data-action="prev-month" aria-label="이전 달" onClick={() => setMonth(new Date(year, index - 1, 1))}><ChevronLeft /></Button>
      <Button variant="outline" size="sm" data-action="today-month" onClick={() => setMonth(new Date(new Date().getFullYear(), new Date().getMonth(), 1))}>오늘</Button>
      <Button variant="outline" size="icon-sm" data-action="next-month" aria-label="다음 달" onClick={() => setMonth(new Date(year, index + 1, 1))}><ChevronRight /></Button>
    </div>
    <div className="calendar">
      {['일', '월', '화', '수', '목', '금', '토'].map((day, idx) => (
        <div key={day} className={`weekday ${idx === 0 ? 'sunday' : idx === 6 ? 'saturday' : ''}`}>{day}</div>
      ))}
      {Array.from({ length: 42 }, (_, i) => {
        const date = new Date(year, index, i - offset + 1);
        const key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
        const entries = data.entries.filter(entry => !entry.deleted_at && (!journalId || entry.journal_id === journalId) && entry.date === key);
        const holiday = holidays[key] || getHoliday(key);
        const colorType = getDayColorType(date, holiday);
        const expanded = expandedDays.has(key);
        return <div key={key} className={`day ${colorType} ${date.getMonth() === index ? '' : 'outside'} ${key === localDate() ? 'today' : ''}`} data-date={key}>
          <button type="button" className="day-content" data-action="calendar-day"
            aria-label={`${dateLabel(key)} 일기 ${entries.length}개 보기`}
            onClick={() => void go(`${journalId ? `/journals/${journalId}` : '/all'}?date=${key}`)}>
            <div className="day-head">
              <span className="day-number">{date.getDate()}</span>
              {holiday && <span className="day-holiday" title={holiday.localName}>{holiday.localName}</span>}
            </div>
            <div className="day-entries" id={`calendar-entries-${key}`}>
              {(expanded ? entries : entries.slice(0, 2)).map(entry => <em key={entry.id} title={entry.title || '제목 없는 일기'}>● {entry.title || '제목 없는 일기'}</em>)}
            </div>
          </button>
          {entries.length > 2 && <button type="button" className="day-more" data-action="calendar-more"
            aria-expanded={expanded} aria-controls={`calendar-entries-${key}`}
            aria-label={`${dateLabel(key)} 일기 ${expanded ? '접기' : `${entries.length - 2}개 더보기`}`}
            onClick={() => setExpandedDays(previous => {
              const next = new Set(previous);
              if (next.has(key)) next.delete(key); else next.add(key);
              return next;
            })}>{expanded ? '접기' : `+${entries.length - 2}개 더보기`}</button>}
        </div>;
      })}
    </div>
  </div>;
}
