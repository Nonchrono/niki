import { useEffect, useRef, useState } from 'react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { NativeSelect, NativeSelectOption } from '../components/ui/native-select';
import { errorMessage } from '../lib/format';
import { Modal } from './Modal';
import { activeAccount, type ChatGPTStatus } from './types';

export function ChatGPTSettings() {
  const { run, action, toast } = useApp();
  const [status, setStatus] = useState<ChatGPTStatus | null>(null);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [welcome, setWelcome] = useState(false);
  const pending = useRef<string | null>(null);
  const mounted = useRef(true);
  const account = activeAccount(status);
  const load = async () => { const value = await run<ChatGPTStatus>('chatgpt_status'); if (mounted.current) setStatus(value); return value; };
  useEffect(() => {
    mounted.current = true;
    void load().catch(e => { if (mounted.current) setError(errorMessage(e)); });
    return () => { mounted.current = false; if (pending.current) void run('chatgpt_cancel', { requestId: pending.current }).catch(() => {}); };
  }, [run]);
  const connect = async (clientId: string | null) => {
    if (pending.current || busy) return;
    const requestId = crypto.randomUUID(); pending.current = requestId;
    setBusy(true); setCancelling(false); setError(''); setNotice('');
    try {
      const value = await run<ChatGPTStatus>('chatgpt_connect', { requestId, clientId });
      if (!mounted.current) return;
      setStatus(value);
      const selected = activeAccount(value);
      if (selected?.plan_enabled && !selected.welcome_seen) setWelcome(true);
      else if (!selected?.plan_enabled) setError('로그인됐지만 ChatGPT 구독 사용 권한이 없습니다. 아래 버튼으로 구독 사용을 허용해 주세요.');
      else toast('ChatGPT 계정이 연결됐습니다');
    } catch (e) { if (mounted.current) setError(errorMessage(e)); }
    finally { pending.current = null; if (mounted.current) { setBusy(false); setCancelling(false); } }
  };
  const cancel = () => { if (!pending.current || cancelling) return; setCancelling(true); void action(() => run('chatgpt_cancel', { requestId: pending.current })); };
  const acknowledge = () => { setWelcome(false); void action(async () => { await run('chatgpt_acknowledge'); await load(); }); };
  return <>
    <div className="wide-head"><h2>ChatGPT 연결</h2></div>
    <div className="panel-card settings-card chatgpt-settings">
      <h3>내 ChatGPT 구독으로 일기 요약</h3>
      <p>GPT-6 Luna가 선택한 일기 한 편을 요약합니다. 요약할 때 제목·날짜·본문을 OpenAI에 전송하며, 첨부 파일의 내용은 전송하지 않습니다.</p>
      <p>별도의 API 키 없이 연결합니다. ChatGPT 구독의 Work·Codex 사용량을 공유하며, 구독과 앱별 한도가 적용됩니다. Niki는 AI 요약에 별도 요금을 청구하지 않습니다.</p>
      {status && status.accounts.length > 0 && <label className="chatgpt-account-label">사용할 ChatGPT 계정
        <NativeSelect aria-label="사용할 ChatGPT 계정" value={status.active ?? ''} disabled={busy} onChange={event => {
          const clientId = event.target.value; setBusy(true); setError('');
          void run<ChatGPTStatus>('chatgpt_select_account', { clientId }).then(value => { if (mounted.current) setStatus(value); }).catch(e => { if (mounted.current) setError(errorMessage(e)); }).finally(() => { if (mounted.current) setBusy(false); });
        }}>{status.accounts.map((a, index) => <NativeSelectOption key={a.client_id} value={a.client_id}>{a.email || 'ChatGPT 계정'} · 계정 {index + 1}{a.connected ? '' : ' (로그아웃)'}</NativeSelectOption>)}</NativeSelect>
      </label>}
      <div className="chatgpt-connection" role="status">{account?.connected ? account.plan_enabled ? '연결됨 · ChatGPT 구독 사용 중 · GPT-6 Luna' : '로그인됨 · 구독 사용 허용 필요' : status ? '연결된 ChatGPT 계정이 없습니다' : '연결 상태 확인 중…'}</div>
      {error && <p className="chatgpt-error" role="alert">{error}</p>}
      {notice && <p role="status">{notice}</p>}
      {busy && <p role="status">{pending.current ? cancelling ? '로그인을 취소하고 있습니다…' : '브라우저에서 ChatGPT 로그인을 완료해 주세요. 등록 후 계정 확인 화면이 한 번 더 열릴 수 있습니다. 각 로그인은 최대 4분 동안 기다립니다.' : '연결 정보를 갱신하고 있습니다…'}</p>}
      <div className="panel-actions">
        {!account?.connected || !account.plan_enabled ? <Button className="bg-[#111] text-white hover:bg-[#333]" data-oai-siwc-button="" data-action="chatgpt-connect" disabled={busy || !status} onClick={() => void connect(account?.client_id ?? null)}>Continue with ChatGPT</Button> : null}
        {status && status.accounts.length > 0 && <Button variant="outline" data-action="chatgpt-add-account" disabled={busy} onClick={() => void connect(null)}>다른 ChatGPT 계정 추가</Button>}
        {account?.connected && <Button variant="outline" data-action="chatgpt-disconnect" disabled={busy} onClick={() => {
          setBusy(true); setError('');
          void run<string>('chatgpt_disconnect').then(async message => { if (mounted.current) { setNotice(message); await load(); } }).catch(e => { if (mounted.current) setError(errorMessage(e)); }).finally(() => { if (mounted.current) setBusy(false); });
        }}>연결 해제</Button>}
        {pending.current && <Button variant="outline" data-action="chatgpt-cancel-login" disabled={cancelling} onClick={cancel}>로그인 취소</Button>}
        <Button variant="outline" data-action="chatgpt-usage" onClick={() => void action(() => run('chatgpt_open_usage'))}>사용량 관리</Button>
        {!status && error && <Button variant="outline" onClick={() => { setError(''); void load().catch(e => setError(errorMessage(e))); }}>다시 확인</Button>}
      </div>
      <p className="subtle">연결 정보는 이 기기에 보호해서 저장되며 일기 백업과 Google Drive 동기화에 포함되지 않습니다. 다른 기기에서는 따로 연결해 주세요.</p>
    </div>
    {welcome && <Modal title="ChatGPT 구독을 사용합니다" close={acknowledge}>
      <p>Niki의 일기 요약은 연결한 ChatGPT 구독의 Work·Codex 사용량을 사용합니다. 사용량과 앱별 한도는 ChatGPT 설정에서 관리할 수 있습니다.</p>
      <div className="panel-actions"><Button variant="outline" onClick={() => void action(() => run('chatgpt_open_usage'))}>사용량 관리</Button><Button data-action="chatgpt-welcome-ok" onClick={acknowledge}>확인</Button></div>
    </Modal>}
  </>;
}
