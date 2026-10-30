import { useEffect, useState } from 'react';
import { X } from 'lucide-react';
import { Button } from '../components/ui/button';
import { useApp } from '../app/AppContext';
import type { Attachment } from '../types';

export function AttachmentPreview({ attachment, onRemove }: { attachment: Attachment; onRemove: () => void }) {
  const { media, action } = useApp();
  const [url, setUrl] = useState('');
  const [open, setOpen] = useState(false);
  useEffect(() => {
    let live = true;
    if (attachment.mime.startsWith('image/')) void media(attachment.id).then(value => { if (live) setUrl(value); }).catch(console.warn);
    return () => { live = false; };
  }, [attachment.id, attachment.mime, media]);
  const show = () => void action(async () => { if (!url) setUrl(await media(attachment.id)); setOpen(true); });
  return <>
    <div className="attach-tile" id={`att-${attachment.id}`} onClick={show}>
      {url && attachment.mime.startsWith('image/') ? <img src={url} alt={attachment.name} /> : <span>{attachment.name}</span>}
      <button data-action="remove-attachment" title="삭제" aria-label="첨부 삭제" onClick={event => { event.stopPropagation(); onRemove(); }}><X size={14} /></button>
    </div>
    {open && <div className="media-overlay" onClick={() => setOpen(false)}><div className="media-dialog" onClick={event => event.stopPropagation()}>
      <div className="media-dialog-head"><strong>{attachment.name}</strong><Button variant="outline" size="sm" onClick={() => setOpen(false)}>닫기</Button></div>
      <div className="media-content">
        {attachment.mime.startsWith('image/') ? <img src={url} alt={attachment.name} />
          : attachment.mime.startsWith('video/') ? <video src={url} controls autoPlay />
          : attachment.mime.startsWith('audio/') ? <audio src={url} controls autoPlay />
          : attachment.mime === 'application/pdf' ? <iframe src={url} title={attachment.name} />
          : <p>미리보기를 지원하지 않는 파일입니다.</p>}
      </div><a className="ghost" href={url} download={attachment.name}>파일 저장</a>
    </div></div>}
  </>;
}
