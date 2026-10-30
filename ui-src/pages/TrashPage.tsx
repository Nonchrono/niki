import { RotateCcw, Trash2 } from 'lucide-react';
import { Button } from '../components/ui/button';
import { Card } from '../components/ui/card';
import { useApp } from '../app/AppContext';
import { dateLabel } from '../lib/format';

export function TrashPage() {
  const { data, run, refresh, action, clearMedia, toast } = useApp();
  if (!data) return null;
  const items = data.entries.filter(entry => entry.deleted_at);
  return <div className="wide-view"><div className="wide-head"><h2>휴지통</h2>
    <span className="subtle">{items.length}개</span><span className="spacer" />
    {items.length > 0 && <Button variant="outline" size="sm" className="text-destructive" data-action="empty-trash"
      onClick={() => void action(async () => {
        if (!confirm('휴지통의 모든 기록을 영구 삭제할까요? 삭제 후에는 복원할 수 없습니다.')) return;
        const count = await run<number>('empty_trash');
        clearMedia(); await refresh(); toast(`휴지통에서 ${count}개의 기록을 영구 삭제했습니다`);
      })}><Trash2 />휴지통 비우기</Button>}
  </div>
    {items.length ? items.map(entry => <Card className="panel-card gap-2" key={entry.id}>
      <span className="subtle">{dateLabel(entry.date)}</span><h3>{entry.title || '제목 없는 기록'}</h3>
      <Button variant="outline" size="sm" data-action="restore" onClick={() => void action(async () => {
        await run('restore_entry', { id: entry.id }); await refresh(); toast('기록을 복원했습니다');
      })}><RotateCcw />복원</Button>
    </Card>) : <div className="empty"><p>휴지통이 비어 있습니다.</p></div>}
  </div>;
}
