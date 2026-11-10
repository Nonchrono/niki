import { useEffect, useRef, useState } from 'react';
import { Sparkles } from 'lucide-react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { errorMessage } from '../lib/format';
import { Modal } from './Modal';
import { activeAccount, type ChatGPTStatus, type DiarySummary } from './types';

export function SummaryButton({ prepare }: { prepare: () => Promise<{ id: string; title: string; updatedAt: string } | null> }) {
  const { run, action, go } = useApp();
  const [open, setOpen] = useState(false);
  const [source, setSource] = useState<{ id: string; title: string; updatedAt: string } | null>(null);
  const [connection, setConnection] = useState<ChatGPTStatus | null>(null);
  const [result, setResult] = useState<DiarySummary | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [opening, setOpening] = useState(false);
  const [copied, setCopied] = useState(false);
  const pending = useRef<string | null>(null);
  const generation = useRef(0);
  const preparing = useRef(false);
  const account = activeAccount(connection);
  const usageLimited = /사용량 한도|요청 한도|subscription_sharing_usage_limit_exceeded/.test(error);
  const cancel = () => { if (pending.current) { void run('chatgpt_cancel', { requestId: pending.current }).catch(() => {}); pending.current = null; } };
  useEffect(() => () => { generation.current++; cancel(); }, [run]);
  const close = () => { generation.current++; cancel(); setOpen(false); setBusy(false); setResult(null); setSource(null); };
  const show = async () => {
    if (preparing.current) return;
    preparing.current = true; setOpening(true);
    const version = ++generation.current;
    try {
      const saved = await prepare();
      if (!saved || version !== generation.current) return;
      setSource(saved); setResult(null); setError(''); setConnection(null); setCopied(false); setOpen(true);
      const status = await run<ChatGPTStatus>('chatgpt_status');
      if (version === generation.current) setConnection(status);
    } catch (e) { if (version === generation.current) setError(errorMessage(e)); }
    finally { preparing.current = false; setOpening(false); }
  };
  const summarize = async () => {
    if (!source || pending.current) return;
    const requestId = crypto.randomUUID(); pending.current = requestId;
    const version = generation.current;
    setBusy(true); setError(''); setResult(null); setCopied(false);
    try {
      const summary = await run<DiarySummary>('chatgpt_summarize', { requestId, entryId: source.id, expectedUpdatedAt: source.updatedAt });
      if (version === generation.current) setResult(summary);
    } catch (e) { if (version === generation.current) setError(errorMessage(e)); }
    finally { if (pending.current === requestId) pending.current = null; if (version === generation.current) setBusy(false); }
  };
  return <>
    <Button variant="outline" size="sm" data-action="summarize-entry" disabled={opening} onClick={() => void show()}><Sparkles size={15} />{opening ? '준비 중…' : '일기 요약'}</Button>
    {open && <Modal title="선택한 일기 요약" close={close}>
      <p className="chatgpt-summary-title">{source?.title.trim() || '제목 없는 일기'}</p>
      <p className="subtle">GPT-6 Luna{account?.plan_enabled ? ' · ChatGPT 구독 사용 중' : ''}</p>
      <p>이 일기의 제목·날짜·본문을 OpenAI로 보내 요약합니다. 첨부 파일의 내용은 전송하지 않으며 원본 일기는 변경하지 않습니다.</p>
      {connection && !account?.plan_enabled && <p className="chatgpt-error">설정에서 ChatGPT를 연결하고 구독 사용을 허용해 주세요.</p>}
      {!connection && !error && <p role="status">ChatGPT 연결 상태 확인 중…</p>}
      {busy && <p className="chatgpt-summary-loading" role="status">일기를 요약하고 있습니다…</p>}
      {error && <p className="chatgpt-error" role="alert">{error}</p>}
      {result && <div className="chatgpt-summary-result" role="region" aria-label="일기 요약 결과" tabIndex={0}>{result.text}</div>}
      {result && <p className="subtle">AI 요약에는 오류가 있을 수 있습니다. 원문과 함께 확인해 주세요.</p>}
      <div className="panel-actions">
        <Button variant="outline" data-action="summary-close" onClick={close}>{busy ? '취소하고 닫기' : '닫기'}</Button>
        <Button variant={usageLimited ? 'default' : 'outline'} data-action="summary-usage" onClick={() => void action(() => run('chatgpt_open_usage'))}>사용량 관리</Button>
        {connection && !account?.plan_enabled ? <Button data-action="summary-connect" onClick={() => { close(); void go('/settings/chatgpt'); }}>ChatGPT 연결</Button> : !result && <Button variant={usageLimited ? 'outline' : 'default'} data-action="summary-start" disabled={!account?.plan_enabled || busy} onClick={() => void summarize()}>{busy ? '요약 중…' : '이 일기 요약하기'}</Button>}
        {result && <Button data-action="summary-copy" onClick={() => void action(async () => { await navigator.clipboard.writeText(result.text); setCopied(true); })}>{copied ? '복사됨' : '요약 복사'}</Button>}
      </div>
      {busy && <p className="subtle">취소하기 전에 처리된 요청은 ChatGPT 사용량에 포함될 수 있습니다.</p>}
    </Modal>}
  </>;
}
