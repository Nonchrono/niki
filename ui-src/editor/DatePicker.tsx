import { useState } from 'react';
import { CalendarDays, ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { dateLabel, localDate } from '../lib/format';

export function DatePicker({ value, onChange }: { value: string; onChange: (date: string) => void }) {
  const [open, setOpen] = useState(false);
  const [month, setMonth] = useState(() => new Date(`${value}T12:00:00`));
  const year = month.getFullYear(), monthIndex = month.getMonth();
  const offset = new Date(year, monthIndex, 1).getDay();
  const choose = (date: string) => { onChange(date); setOpen(false); };
  return <span className="date-control">
    <Button variant="outline" size="xs" className="date-trigger" data-action="date-picker" aria-label="기록 날짜 선택"
      aria-haspopup="dialog" onClick={() => { setMonth(new Date(`${value}T12:00:00`)); setOpen(!open); }}>
      <CalendarDays size={13} />{dateLabel(value)}<ChevronDown size={12} />
    </Button>
    {open && <div className="date-popover" role="dialog" aria-label="날짜 선택">
      <div className="date-header">
        <Button variant="ghost" size="icon-xs" className="date-nav" data-action="date-prev" aria-label="이전 달"
          onClick={() => setMonth(new Date(year, monthIndex - 1, 1))}><ChevronLeft /></Button>
        <strong>{year}년 {monthIndex + 1}월</strong>
        <Button variant="ghost" size="icon-xs" className="date-nav" data-action="date-next" aria-label="다음 달"
          onClick={() => setMonth(new Date(year, monthIndex + 1, 1))}><ChevronRight /></Button>
      </div>
      <div className="date-grid">
        {['일', '월', '화', '수', '목', '금', '토'].map(day => <span key={day} className="date-weekday">{day}</span>)}
        {Array.from({ length: 42 }, (_, i) => {
          const date = new Date(year, monthIndex, i - offset + 1);
          const key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
          return <button key={i} className={`date-day ${date.getMonth() !== monthIndex ? 'outside ' : ''}${key === value ? 'selected ' : ''}${key === localDate() ? 'today' : ''}`}
            data-action="date-select" aria-label={dateLabel(key)} aria-current={key === value ? 'date' : undefined}
            onClick={() => choose(key)}>{date.getDate()}</button>;
        })}
      </div>
      <label className="date-direct">날짜 직접 선택 <Input id="date-direct" type="date" value={value} onChange={event => choose(event.target.value)} /></label>
      <Button variant="ghost" size="xs" className="date-today" onClick={() => choose(localDate())}>오늘</Button>
    </div>}
  </span>;
}
