import { useEffect, useRef, useState } from 'react';
import { MessageSquare, X } from 'lucide-react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { errorMessage } from '../lib/format';

export function SummaryPanel({ entryId, prepare, onClose }: { entryId: string; prepare: () => Promise<string | null>; onClose: () => void }) {
  const { run } = useApp();
  const active = useRef(true);
  const [busy, setBusy] = useState(false);
  const [summary, setSummary] = useState('');
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    active.current = true;
    return () => { active.current = false; };
  }, [run]);
  const summarize = async () => {
    setBusy(true); setError(''); setSummary(''); setCopied(false);
    try {
      const expectedUpdatedAt = await prepare();
      if (!expectedUpdatedAt || !active.current) return;
      const result = await run<string>('codex_summarize', { entryId, expectedUpdatedAt });
      if (active.current) setSummary(result);
    } catch (e) { if (active.current) setError(errorMessage(e)); }
    finally { if (active.current) setBusy(false); }
  };
  return <aside className="codex-summary" id="diary-summary-sidebar" aria-label="선택한 일기 요약">
    <div className="codex-summary-head"><div><h3>일기 요약</h3><span className="subtle">GPT-6 Luna</span></div><Button variant="ghost" size="icon-sm" aria-label="요약 닫기" onClick={onClose}><X size={17} /></Button></div>
    <div className="codex-conversation" role="log" aria-label="요약 대화" aria-live="polite" aria-busy={busy}>
      {!busy && !summary && !error && <div className="codex-conversation-empty"><MessageSquare size={25} strokeWidth={1.4} /><p>일기의 이야기를<br />짧게 정리해 보세요.</p></div>}
      {(busy || summary) && <div className="codex-request-bubble">이 일기를 요약해 줘.</div>}
      {busy && <div className="codex-response"><span className="codex-speaker">GPT</span><p role="status" className="codex-summary-text">일기를 읽고 있어요…</p></div>}
      {summary && <div className="codex-response"><span className="codex-speaker">GPT</span><p className="codex-summary-text" data-testid="codex-summary-result">{summary}</p><Button variant="ghost" size="xs" onClick={() => void navigator.clipboard.writeText(summary).then(() => setCopied(true)).catch(() => setError('요약을 복사하지 못했습니다'))}>{copied ? '복사됨' : '요약 복사'}</Button></div>}
      {error && <p role="alert" className="sync-error">{error}</p>}
    </div>
    <div className="codex-summary-actions"><Button disabled={busy} data-action="codex-summarize" onClick={() => void summarize()}>{busy ? '요약 중…' : summary ? '다시 요약하기' : '요약하기'}</Button></div>
  </aside>;
}
