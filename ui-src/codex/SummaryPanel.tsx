import { useEffect, useRef, useState } from 'react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { errorMessage } from '../lib/format';
import type { CodexStatus } from './CodexSettings';

export function SummaryPanel({ entryId, prepare, onClose }: { entryId: string; prepare: () => Promise<string | null>; onClose: () => void }) {
  const { run, go } = useApp();
  const active = useRef(true);
  const [status, setStatus] = useState<CodexStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [summary, setSummary] = useState('');
  const [error, setError] = useState('');
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    active.current = true;
    void run<CodexStatus>('codex_status').then(value => { if (active.current) setStatus(value); }).catch(e => { if (active.current) setError(errorMessage(e)); });
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
  return <section className="codex-summary" aria-label="선택한 일기 요약">
    <div className="codex-summary-head"><h3>일기 요약 <span className="subtle">GPT-6 Luna</span></h3><Button variant="ghost" size="sm" onClick={onClose}>닫기</Button></div>
    <p className="subtle">요약하기를 누르면 편집 내용을 저장하고, 이 일기의 제목과 본문을 OpenAI에 보냅니다. 첨부 파일은 전송하지 않습니다.</p>
    {error && <p role="alert" className="sync-error">{error}</p>}
    {busy && <p role="status">일기를 요약하고 있습니다…</p>}
    {summary && <><p className="codex-summary-text" data-testid="codex-summary-result">{summary}</p><p className="subtle">요청 당시 내용의 AI 요약입니다. 원문과 비교해 확인해 주세요.</p></>}
    <div className="panel-actions">
      {status?.connected ? <Button disabled={busy} data-action="codex-summarize" onClick={() => void summarize()}>{summary ? '다시 요약하기' : '요약하기'}</Button> : status && <Button variant="outline" onClick={() => void go('/settings/codex')}>Codex 연결 설정</Button>}
      {!status && !error && <span className="subtle">연결 상태 확인 중…</span>}
      {summary && <Button variant="outline" onClick={() => void navigator.clipboard.writeText(summary).then(() => setCopied(true)).catch(() => setError('요약을 복사하지 못했습니다'))}>{copied ? '복사됨' : '요약 복사'}</Button>}
    </div>
  </section>;
}
