import { useEffect, useRef, useState } from 'react';
import { ArrowUp, FileText, MessageSquare, Pin, PinOff, RotateCcw, Square, SquarePen, X } from 'lucide-react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { askConfirm } from '../lib/dialog';
import { useChat, type ChatMessage } from './ChatContext';
import { crisisContacts, detectCrisis } from './safety';

const starters = [
  '요즘 내 감정 흐름은 어때?',
  '작년 이맘때 나는 어떻게 지냈어?',
  '최근에 힘들었던 일을 같이 돌아보고 싶어',
];

function CrisisCard() {
  return <div className="codex-crisis" role="alert" data-testid="crisis-card">
    <strong>지금 많이 힘드시군요. 혼자 견디지 않으셔도 돼요.</strong>
    <p>바로 이야기할 수 있는 곳이에요. 모두 무료이고 비밀이 보장돼요.</p>
    <ul>{crisisContacts.map(contact => <li key={contact.number}><a href={`tel:${contact.number.replace(/-/g, '')}`}>{contact.number}</a> {contact.name} <span>· {contact.note}</span></li>)}</ul>
  </div>;
}

function quoteForDiary(content: string) {
  const lines = content.trim().split('\n').map(line => `> ${line}`.trimEnd());
  return `> **Niki의 이야기**\n>\n${lines.join('\n')}`;
}

export function SummaryPanel() {
  const { open, close, messages, organizing, model, pending, busy, error, ready, stage, streamText, draft, setDraft,
    send, summarize, cancel, regenerate, clearConversation, target, reload, prefs, setConsent, pinned, setPinned } = useChat();
  const { go, data, toast } = useApp();
  const conversation = useRef<HTMLDivElement>(null);
  const composer = useRef<HTMLTextAreaElement>(null);
  const [copied, setCopied] = useState('');
  const [copyError, setCopyError] = useState('');
  const [consenting, setConsenting] = useState(false);
  useEffect(() => { conversation.current?.scrollTo({ top: conversation.current.scrollHeight }); }, [messages, pending, busy, error, streamText]);
  useEffect(() => {
    const field = composer.current;
    if (!field) return;
    field.style.height = 'auto';
    field.style.height = `${Math.min(field.scrollHeight, 132)}px`;
  }, [draft, open]);
  const needsConsent = !!prefs && !prefs.consented;
  const lastAnswer = [...messages].reverse().find(message => message.role === 'assistant');
  const canRegenerate = !busy && lastAnswer?.kind === 'chat' && messages[messages.length - 1] === lastAnswer;
  const entryExists = (id: string) => !!data?.entries.some(entry => entry.id === id && !entry.deleted_at);
  const startNew = async () => {
    if (!messages.length) return;
    if (await askConfirm('지금까지의 대화와 대화 요약을 이 기기에서 지우고 새로 시작할까요? 일기는 지워지지 않아요.', { okLabel: '새로 시작', danger: true })) await clearConversation();
  };
  const copy = (message: ChatMessage) => void navigator.clipboard.writeText(message.content)
    .then(() => { setCopied(message.id); setCopyError(''); })
    .catch(() => setCopyError('대화를 복사하지 못했습니다.'));
  const addToDiary = (message: ChatMessage) => {
    if (target?.append?.(quoteForDiary(message.content))) toast('열려 있는 일기 끝에 붙였어요');
    else toast('이 일기에는 붙일 수 없어요. 일기를 연 상태에서 서식 편집기를 사용해 주세요.', true);
  };
  const submit = () => { if (!busy && ready && draft.trim()) void send(); };
  return <aside className={`codex-summary${open ? ' open' : ''}`} id="diary-summary-sidebar" aria-label="Niki와 대화" aria-hidden={!open}>
    <div className="codex-summary-head">
      <div className="codex-summary-title"><h3>Niki와 대화</h3>
        <Button variant="outline" size="xs" data-action="codex-summarize" disabled={busy || !ready || needsConsent || !target || target.disabled}
          title={target ? '현재 열려 있는 일기 요약' : '요약할 일기를 먼저 열어 주세요'} onClick={() => void summarize()}>요약</Button>
      </div>
      <div className="codex-head-actions">
        <Button variant="ghost" size="icon-sm" data-action="codex-new-chat" title="새 대화" aria-label="새 대화" disabled={busy || !messages.length} onClick={() => void startNew()}><SquarePen size={16} /></Button>
        <Button variant="ghost" size="icon-sm" data-action="codex-pin" title={pinned ? '고정 해제 (일기 위에 겹쳐 보기)' : '고정 (일기를 옆으로 밀어 함께 보기)'} aria-label={pinned ? '고정 해제' : '패널 고정'} aria-pressed={pinned} onClick={() => setPinned(!pinned)}>{pinned ? <PinOff size={16} /> : <Pin size={16} />}</Button>
        <Button variant="ghost" size="icon-sm" aria-label="대화 닫기" onClick={close}><X size={17} /></Button>
      </div>
    </div>
    {model && !model.installed && (model.downloading || model.failed) && <p className="codex-memory-status" role="status" data-testid="memory-model-status">
      {model.downloading
        ? `일기 검색을 준비하고 있어요… ${Math.min(99, Math.floor(model.downloaded_bytes / Math.max(1, model.total_bytes) * 100))}%`
        : '일기 검색 준비가 끝나지 않아 지금은 단어로 일기를 찾아요. 잠시 후 다시 시도할게요.'}
    </p>}
    {organizing && !needsConsent && <p className="codex-memory-status" role="status">지난 일기와 대화를 정리하며 기억하는 중이에요. 메시지를 보내면 바로 대화할 수 있어요.</p>}
    {needsConsent ? <div className="codex-consent" data-testid="consent-card">
      <h4>Niki에게 일기를 보여 줄까요?</h4>
      <p>Niki가 일기를 바탕으로 이야기하려면 일기의 제목과 본문 일부가 <strong>OpenAI</strong>로 전송돼요. 최근 일기와 대화에 관련된 일기의 원문, 오래된 일기의 앞부분과 월별 요약이 함께 나가요.</p>
      <ul>
        <li>보낸 내용은 내 ChatGPT 계정으로 처리되고, Niki는 별도 서버를 거치지 않아요.</li>
        <li>일기를 찾는 검색은 이 기기 안에서 해요.</li>
        <li>숨기고 싶은 일기장은 왼쪽 일기장 목록에서 우클릭해 <strong>Niki에게 숨기기</strong>를 고르면 돼요.</li>
        <li>설정의 'Niki와 대화'에서 언제든 동의를 거둘 수 있어요.</li>
      </ul>
      <Button data-action="codex-consent" disabled={consenting} onClick={() => { setConsenting(true); void setConsent(true).finally(() => setConsenting(false)); }}>동의하고 시작하기</Button>
    </div> : <>
      <div ref={conversation} className="codex-conversation" role="log" aria-label="Niki 대화" aria-live="polite" aria-busy={busy}>
        {!ready && !error && <p role="status">대화를 불러오는 중…</p>}
        {ready && !busy && !messages.length && <div className="codex-conversation-empty">
          <MessageSquare size={25} strokeWidth={1.4} />
          <p>Niki에게 이야기를 들려주세요.<br />열려 있는 일기를 요약할 수도 있어요.</p>
          <div className="codex-starters">
            {target && !target.disabled && <button type="button" className="codex-starter" data-action="codex-starter-summary" onClick={() => void summarize()}><FileText size={13} />열려 있는 일기 요약</button>}
            {starters.map(text => <button type="button" key={text} className="codex-starter" data-action="codex-starter" onClick={() => void send(text)}>{text}</button>)}
          </div>
        </div>}
        {messages.map(message => message.role === 'user'
          ? <div key={message.id} className="codex-turn">
            <div className="codex-request-bubble">{message.content}</div>
            {message.kind === 'chat' && detectCrisis(message.content) && <CrisisCard />}
          </div>
          : <div key={message.id} className="codex-response"><span className="codex-speaker">Niki</span>
            <p className="codex-summary-text" data-testid={message.kind === 'summary' ? 'codex-summary-result' : 'codex-chat-result'}>{message.content}</p>
            {!!message.sources?.length && <div className="codex-sources" data-testid="codex-sources">
              <span>참고한 일기</span>
              {message.sources.map(source => <button type="button" key={source.id} className="codex-source" data-action="codex-source" disabled={!entryExists(source.id)}
                title={entryExists(source.id) ? '이 일기 열기' : '삭제되었거나 찾을 수 없는 일기'} onClick={() => void go(`/journals/${source.journal_id}/entries/${source.id}`)}>
                {source.date} · {source.title || '제목 없는 일기'}</button>)}
            </div>}
            <div className="codex-actions">
              <Button variant="ghost" size="xs" onClick={() => copy(message)}>{copied === message.id ? '복사됨' : '복사'}</Button>
              {message.kind === 'chat' && target?.append && !target.disabled && <Button variant="ghost" size="xs" data-action="codex-add-to-diary" onClick={() => addToDiary(message)}>일기에 붙이기</Button>}
              {canRegenerate && message === lastAnswer && <Button variant="ghost" size="xs" data-action="codex-regenerate" onClick={() => void regenerate()}><RotateCcw size={12} />다시 답변받기</Button>}
            </div></div>)}
        {pending && <div className="codex-turn"><div className="codex-request-bubble">{pending}</div>{detectCrisis(pending) && <CrisisCard />}</div>}
        {busy && <div className="codex-response"><span className="codex-speaker">Niki</span>
          {streamText
            ? <p className="codex-summary-text" data-testid="codex-streaming">{streamText}</p>
            : <p role="status" className="codex-summary-text codex-waiting" data-testid="codex-stage">{stage === 'searching' ? '관련된 지난 일기를 찾고 있어요…' : '답변을 쓰고 있어요…'}</p>}
          <div className="codex-actions"><Button variant="outline" size="xs" data-action="codex-cancel" onClick={cancel}><Square size={11} />멈추기</Button></div>
        </div>}
        {(error || copyError) && <p role="alert" className="sync-error">{error || copyError}</p>}
        {!ready && error && <Button variant="outline" size="sm" onClick={() => void reload()}>대화 다시 불러오기</Button>}
      </div>
      <form className="codex-chat-composer" onSubmit={event => { event.preventDefault(); submit(); }}>
        <textarea ref={composer} className="codex-input" rows={1} aria-label="Niki에게 메시지" placeholder="Niki에게 메시지 보내기 (Shift+Enter로 줄바꿈)" value={draft} maxLength={6000} disabled={busy}
          onChange={event => setDraft(event.target.value)}
          onKeyDown={event => {
            if (event.key !== 'Enter' || event.shiftKey) return;
            // Enter while composing Hangul only confirms the syllable.
            if (event.nativeEvent.isComposing || event.keyCode === 229) { event.preventDefault(); return; }
            event.preventDefault(); submit();
          }} />
        <Button type="submit" size="icon-sm" aria-label="메시지 보내기" disabled={busy || !ready || !draft.trim()}><ArrowUp size={17} /></Button>
      </form>
      <p className="codex-footnote">대화는 이 기기에만 저장되고 다른 기기와 동기화되지 않아요.</p>
    </>}
  </aside>;
}
