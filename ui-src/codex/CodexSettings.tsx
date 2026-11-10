import { useEffect, useState } from 'react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { errorMessage } from '../lib/format';

export type CodexStatus = { installed: boolean; connected: boolean; plan: string | null; login_pending: boolean };
type Login = { verification_url: string; user_code: string };

export function CodexSettings() {
  const { run } = useApp();
  const [status, setStatus] = useState<CodexStatus | null>(null);
  const [login, setLogin] = useState<Login | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const check = async () => {
      try {
        const next = await run<CodexStatus>('codex_status');
        if (!active) return;
        setStatus(next);
        if (next.connected) setLogin(null);
        if (next.login_pending) timer = setTimeout(check, 2000);
      } catch (e) { if (active) { setError(errorMessage(e)); setLogin(null); } }
    };
    void check();
    return () => { active = false; clearTimeout(timer); };
  }, [run, login?.user_code]);
  const perform = async (work: () => Promise<void>) => {
    setBusy(true); setError('');
    try { await work(); } catch (e) { setError(errorMessage(e)); }
    finally { setBusy(false); }
  };
  const disconnect = () => void perform(async () => {
    await run('codex_disconnect'); setLogin(null);
    setStatus(await run<CodexStatus>('codex_status'));
  });
  return <>
    <div className="wide-head"><h2>일기 요약</h2></div>
    <div className="panel-card settings-card codex-settings">
      <h3>Codex 계정 연결</h3>
      <p>ChatGPT 계정으로 연결하고 GPT-6 Luna로 선택한 일기를 요약합니다. 사용 가능 여부와 한도는 계정에 따라 달라집니다.</p>
      <p className="subtle">이 PC에 Codex가 설치되어 있어야 합니다. Niki의 로그인은 기존 Codex 로그인과 별도로 관리합니다.</p>
      {status?.connected ? <p role="status">연결됨 · {status.plan || '플랜 확인 불가'}</p> : <p role="status">{status ? status.installed ? '계정 연결이 필요합니다' : 'Codex를 설치한 뒤 다시 확인해 주세요 (CLI 0.167 이상).' : '연결 상태 확인 중…'}</p>}
      {error && <p className="sync-error" role="alert">{error}</p>}
      {login && <div className="codex-login">
        <p>브라우저에서 ChatGPT에 로그인한 뒤 아래 코드를 입력하세요.</p>
        <code className="codex-device-code">{login.user_code}</code>
        <p className="subtle">{login.verification_url}</p>
        <div className="panel-actions">
          <Button disabled={busy} onClick={() => void perform(async () => { await run('codex_open_login'); })}>로그인 페이지 열기</Button>
          <Button variant="outline" disabled={busy} onClick={() => void perform(() => navigator.clipboard.writeText(login.user_code))}>코드 복사</Button>
        </div>
        <p className="subtle">로그인이 완료되면 자동으로 연결됩니다. 기기 코드 인증 안내가 나오면 ChatGPT 보안 설정을 확인해 주세요.</p>
      </div>}
      <div className="panel-actions">
        {!status?.connected && !login && !status?.login_pending && <Button disabled={busy || !status?.installed} data-action="codex-connect" onClick={() => void perform(async () => { setLogin(await run<Login>('codex_connect')); })}>ChatGPT 계정으로 연결</Button>}
        {(status?.connected || login || status?.login_pending) && <Button variant="outline" disabled={busy} data-action="codex-disconnect" onClick={disconnect}>{status?.connected ? '연결 해제' : '로그인 취소'}</Button>}
        {!login && <Button variant="outline" disabled={busy} onClick={() => void perform(async () => setStatus(await run<CodexStatus>('codex_status')))}>상태 새로고침</Button>}
      </div>
    </div>
    <div className="panel-card settings-card">
      <h3>전송되는 정보</h3>
      <p>일기에서 ‘요약하기’를 누르면 해당 일기의 제목과 본문이 OpenAI로 전송됩니다. 첨부 파일과 다른 일기는 보내지 않습니다.</p>
      <p>로그인 정보는 OS 자격 증명 저장소에 보관하며 일기 백업이나 Google Drive 동기화에 포함하지 않습니다. 요약은 화면에만 표시하고 원문을 변경하지 않습니다.</p>
    </div>
  </>;
}
