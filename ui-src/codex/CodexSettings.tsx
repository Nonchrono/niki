import { useEffect, useState } from 'react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { errorMessage } from '../lib/format';
import { GPT_CONNECTION_CHANGED } from './useGptConnection';

export type CodexStatus = { installed: boolean; connected: boolean; plan: string | null; login_pending: boolean };
type Login = { verification_url: string; user_code: string };

export function CodexSettings() {
  const { run } = useApp();
  const [status, setStatus] = useState<CodexStatus | null>(null);
  const [login, setLogin] = useState<Login | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => { if (status) window.dispatchEvent(new Event(GPT_CONNECTION_CHANGED)); }, [status?.connected, status?.login_pending]);
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
    <div className="wide-head"><h2>Niki와 대화</h2></div>
    <div className="panel-card settings-card codex-settings">
      <h3>GPT 계정 연결</h3>
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
      <p>요약할 일기, 그리고 Niki와 대화할 때 찾은 일기의 제목과 본문이 OpenAI에 전송됩니다.</p>
      <p>요약 버튼을 누르면 현재 일기를 전송합니다. 메시지를 보내면 입력한 내용, 최근 대화, 그리고 최근 일기와 메시지와 관련된 일기의 원문, 오래된 일기의 앞부분, 월별 개요가 함께 전송됩니다. 관련 일기는 이 기기 안에서 찾습니다. 검색 모델(약 112MB, 대화창에서 받기)을 받으면 의미가 비슷한 일기도 찾으며, 이때 일기는 검색을 위해 외부로 보내지 않습니다. 검색 모델이 없으면 관련 일기를 찾기 위해 메시지와 월별 개요를 담은 짧은 요청을 한 번 더 보냅니다. 월별 개요와 긴 대화의 요약을 만들 때도 해당 일기 앞부분과 대화가 전송되며, 만들어진 요약과 검색 색인은 이 기기에만 저장됩니다. 대화 기록은 이 기기에 저장되어 모든 일기장에서 공통으로 사용합니다.</p>
    </div>
  </>;
}
