import { FileText, Paperclip, RotateCcw, Trash2 } from 'lucide-react';
import { Button } from '../components/ui/button';
import { useApp } from '../app/AppContext';
import { dateLabel } from '../lib/format';
import { askConfirm } from '../lib/dialog';
import { plainText } from '../editor/markdown';

export function TrashPage() {
  const { data, run, refresh, action, clearMedia, toast } = useApp();
  if (!data) return null;
  const items = data.entries.filter(entry => entry.deleted_at);
  return <div className="wide-view trash-view"><div className="wide-head trash-head">
    <div className="trash-heading">
      <div className="trash-title"><h2>삭제된 일기</h2><span className="trash-count">{items.length}개</span></div>
      <p>복원하면 원래 일기장에서 다시 볼 수 있습니다.</p>
    </div>
    {items.length > 0 && <Button variant="outline" size="sm" className="border-destructive/30 bg-transparent text-destructive shadow-none hover:bg-destructive/5 hover:text-destructive" data-action="empty-trash"
      onClick={() => void action(async () => {
        if (!await askConfirm('휴지통의 전체 일기를 영구 삭제할까요? 삭제 후에는 복원할 수 없습니다.', { okLabel: '영구 삭제', danger: true })) return;
        const count = await run<number>('empty_trash');
        clearMedia(); await refresh(); toast(`휴지통에서 ${count}개의 일기를 영구 삭제했습니다`);
      })}><Trash2 />휴지통 비우기</Button>}
  </div>
    {items.length ? <>
      <ul className="trash-list" aria-label="삭제된 일기">
        {items.map(entry => {
          const journal = data.journals.find(item => item.id === entry.journal_id);
          const title = entry.title || '제목 없는 일기';
          const preview = plainText(entry.body.slice(0, 1200));
          return <li className="trash-row" key={entry.id}>
            <div className="trash-record-icon" aria-hidden="true"><FileText /></div>
            <div className="trash-record">
              <h3>{title}</h3>
              <div className="trash-meta">
                <time dateTime={entry.date}>{dateLabel(entry.date)}</time>
                {journal && <span className="trash-journal"><i style={{ background: journal.color }} aria-hidden="true" />{journal.name}</span>}
                {entry.attachments.length > 0 && <span><Paperclip aria-hidden="true" />첨부 {entry.attachments.length}개</span>}
              </div>
              <p className={`trash-preview${preview ? '' : ' is-empty'}`}>{preview || '본문이 없는 일기입니다.'}</p>
            </div>
            <Button variant="outline" size="sm" className="trash-restore border-border bg-white text-primary shadow-none" data-action="restore" aria-label={`${title} 복원`} onClick={() => void action(async () => {
              await run('restore_entry', { id: entry.id }); await refresh(); toast('일기를 복원했습니다');
            })}><RotateCcw />복원</Button>
          </li>;
        })}
      </ul>
      <p className="trash-footnote">휴지통을 비우면 일기가 영구 삭제되며 복원할 수 없습니다.</p>
    </> : <div className="empty trash-empty">
      <div className="empty-art"><Trash2 aria-hidden="true" /></div>
      <h3>휴지통이 비어 있습니다</h3>
      <p>삭제한 일기는 이곳에 보관됩니다.</p>
    </div>}
  </div>;
}
