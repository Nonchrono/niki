import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useApp } from '../app/AppContext';
import type { Attachment } from '../types';

function MediaThumbnail({ attachment }: { attachment: Attachment }) {
  const { media } = useApp(); const [url, setUrl] = useState('');
  useEffect(() => { let live = true; if (attachment.mime.startsWith('image/')) void media(attachment.id).then(value => { if (live) setUrl(value); }).catch(console.warn); return () => { live = false; }; }, [attachment.id, attachment.mime, media]);
  return <div className="media-thumb">{url ? <img src={url} alt={attachment.name} /> : attachment.mime.startsWith('video/') ? '▶' : attachment.mime.startsWith('audio/') ? '♫' : '▧'}</div>;
}
export function MediaPage() {
  const { data, go } = useApp(); if (!data) return null;
  const [params] = useSearchParams(); const journalId = params.get('journal');
  const items = data.entries.filter(entry => !entry.deleted_at && (!journalId || entry.journal_id === journalId)).flatMap(entry => entry.attachments.map(attachment => ({ entry, attachment })));
  return <div className="wide-view"><div className="wide-head"><h2>미디어</h2><span className="subtle">{items.length}개</span></div><div className="media-grid">{items.map(({ entry, attachment }) => <button key={attachment.id} className="media-card" data-action="entry" onClick={() => void go(`/journals/${entry.journal_id}/entries/${entry.id}`)}><MediaThumbnail attachment={attachment} /><span>{attachment.name}</span></button>)}</div></div>;
}
