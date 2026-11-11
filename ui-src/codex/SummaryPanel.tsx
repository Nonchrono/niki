import { useEffect, useRef, useState } from 'react';
import { ArrowUp, MessageSquare, X } from 'lucide-react';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { useChat } from './ChatContext';

export function SummaryPanel() {
  const { open, close, messages, organizing, model, pending, busy, error, ready, draft, setDraft, send, summarize, target, reload } = useChat();
  const conversation = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState('');
  const [copyError, setCopyError] = useState('');
  useEffect(() => { conversation.current?.scrollTo({ top: conversation.current.scrollHeight }); }, [messages, pending, busy, error]);
  return <aside className={`codex-summary${open ? ' open' : ''}`} id="diary-summary-sidebar" aria-label="Niki와 대화" aria-hidden={!open}>
    <div className="codex-summary-head">
      <div className="codex-summary-title"><h3>Niki와 대화</h3>
        <Button variant="outline" size="xs" data-action="codex-summarize" disabled={busy || !ready || !target || target.disabled}
          title={target ? '현재 열려 있는 일기 요약' : '요약할 일기를 먼저 열어 주세요'} onClick={() => void summarize()}>요약</Button>
      </div>
      <Button variant="ghost" size="icon-sm" aria-label="대화 닫기" onClick={close}><X size={17} /></Button>
    </div>
    {model && !model.installed && (model.downloading || model.failed) && <p className="codex-memory-status" role="status" data-testid="memory-model-status">
      {model.downloading
        ? `일기 검색을 준비하고 있어요… ${Math.min(99, Math.floor(model.downloaded_bytes / Math.max(1, model.total_bytes) * 100))}%`
        : '일기 검색 준비가 끝나지 않아 지금은 단어로 일기를 찾아요. 잠시 후 다시 시도할게요.'}
    </p>}
    {organizing && <p className="codex-memory-status" role="status">지난 일기와 대화를 정리하며 기억하는 중이에요. 메시지를 보내면 바로 대화할 수 있어요.</p>}
    <div ref={conversation} className="codex-conversation" role="log" aria-label="Niki 대화" aria-live="polite" aria-busy={busy}>
      {!ready && !error && <p role="status">대화를 불러오는 중…</p>}
      {ready && !busy && !messages.length && <div className="codex-conversation-empty"><MessageSquare size={25} strokeWidth={1.4} /><p>Niki에게 이야기를 들려주세요.<br />열려 있는 일기를 요약할 수도 있어요.</p></div>}
      {messages.map(message => message.role === 'user'
        ? <div key={message.id} className="codex-request-bubble">{message.content}</div>
        : <div key={message.id} className="codex-response"><span className="codex-speaker">Niki</span><p className="codex-summary-text" data-testid={message.kind === 'summary' ? 'codex-summary-result' : 'codex-chat-result'}>{message.content}</p>
          <Button variant="ghost" size="xs" onClick={() => void navigator.clipboard.writeText(message.content).then(() => { setCopied(message.id); setCopyError(''); }).catch(() => setCopyError('대화를 복사하지 못했습니다.'))}>{copied === message.id ? '복사됨' : '복사'}</Button></div>)}
      {pending && <div className="codex-request-bubble">{pending}</div>}
      {busy && <div className="codex-response"><span className="codex-speaker">Niki</span><p role="status" className="codex-summary-text">생각하고 있어요…</p></div>}
      {(error || copyError) && <p role="alert" className="sync-error">{error || copyError}</p>}
      {!ready && error && <Button variant="outline" size="sm" onClick={() => void reload()}>대화 다시 불러오기</Button>}
    </div>
    <form className="codex-chat-composer" onSubmit={event => { event.preventDefault(); void send(); }}>
      <Input aria-label="Niki에게 메시지" placeholder="Niki에게 메시지 보내기" value={draft} maxLength={6000} disabled={busy}
        onChange={event => setDraft(event.target.value)} onKeyDown={event => { if (event.key === 'Enter' && (event.nativeEvent.isComposing || event.keyCode === 229)) event.preventDefault(); }} />
      <Button type="submit" size="icon-sm" aria-label="메시지 보내기" disabled={busy || !ready || !draft.trim()}><ArrowUp size={17} /></Button>
    </form>
  </aside>;
}
