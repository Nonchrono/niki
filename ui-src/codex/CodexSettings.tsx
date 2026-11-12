import { useEffect, useState } from 'react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { errorMessage } from '../lib/format';
import { Input } from '../components/ui/input';
import { NativeSelect, NativeSelectOption } from '../components/ui/native-select';
import { askConfirm } from '../lib/dialog';
import { CODEX_PREFS_CHANGED, GPT_CONNECTION_CHANGED } from './useGptConnection';
import { useChat, type Prefs } from './ChatContext';

export type CodexStatus = { installed: boolean; connected: boolean; plan: string | null; login_pending: boolean };
type Login = { verification_url: string; user_code: string };
type CliInfo = { path: string | null; version: string | null; supported: boolean; custom: boolean; min_version: string };
type MemoryInfo = {
  model_installed: boolean; model_bytes: number; index_bytes: number; indexed_entries: number; total_entries: number;
  downloading: boolean; failed: boolean; months_summarized: number; chat_summarized: boolean;
};
type ModelChoice = { id: string; name: string };
const megabytes = (bytes: number) => `${(bytes / 1_000_000).toFixed(bytes < 10_000_000 ? 1 : 0)}MB`;

function CliCard({ installed }: { installed: boolean | undefined }) {
  const { run } = useApp();
  const [info, setInfo] = useState<CliInfo | null>(null);
  const [path, setPath] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => { void run<CliInfo>('codex_cli_info').then(setInfo).catch(() => undefined); }, [run, installed]);
  const apply = async (value: string | null) => {
    setBusy(true); setError('');
    try {
      setInfo(await run<CliInfo>('codex_set_cli_path', { path: value }));
      setPath('');
      window.dispatchEvent(new Event(GPT_CONNECTION_CHANGED));
    } catch (e) { setError(errorMessage(e)); } finally { setBusy(false); }
  };
  return <div className="panel-card settings-card" data-testid="cli-card">
    <h3>연결 프로그램</h3>
    <p className="subtle">ChatGPT 연결에 사용하는 Codex입니다.</p>
    {info?.path
      ? <>
        <p role="status"><strong>{info.version ? `버전 ${info.version}` : '버전을 확인하지 못했어요'}</strong>{info.supported ? ' · 사용할 수 있어요' : ` · ${info.min_version} 이상이 필요해요. Codex를 업데이트해 주세요.`}</p>
        <p className="subtle codex-path">{info.path}{info.custom ? ' (직접 지정)' : ' (자동 감지)'}</p>
      </>
      : <p role="status">Codex를 찾지 못했어요. Codex 설치 후 Niki를 다시 열어 주세요. 이미 설치했다면 아래에서 파일 위치를 지정할 수 있어요.</p>}
    {error && <p className="sync-error" role="alert">{error}</p>}
    <details className="oauth-details">
      <summary>{info?.path ? '다른 Codex 파일 사용' : 'Codex 경로 직접 지정'}</summary>
      <div className="oauth-details-body">
        <Input aria-label="Codex 실행 파일 전체 경로" placeholder="설치된 Codex 실행 파일 경로" value={path} onChange={event => setPath(event.target.value)} />
        <div className="panel-actions">
          <Button variant="outline" disabled={busy || !path.trim()} data-action="codex-set-path" onClick={() => void apply(path)}>이 경로 사용</Button>
          {info?.custom && <Button variant="outline" disabled={busy} data-action="codex-clear-path" onClick={() => void apply(null)}>자동 감지로 되돌리기</Button>}
        </div>
      </div>
    </details>
  </div>;
}

function PrivacyCard() {
  const { run, data } = useApp();
  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [error, setError] = useState('');
  const load = () => run<Prefs>('codex_prefs').then(setPrefs).catch(e => setError(errorMessage(e)));
  useEffect(() => { void load(); window.addEventListener(CODEX_PREFS_CHANGED, load); return () => window.removeEventListener(CODEX_PREFS_CHANGED, load); }, [run]);
  const change = async (work: () => Promise<Prefs | void>) => {
    setError('');
    try { const next = await work(); if (next) setPrefs(next); window.dispatchEvent(new Event(CODEX_PREFS_CHANGED)); } catch (e) { setError(errorMessage(e)); }
  };
  return <div className="panel-card settings-card" data-testid="privacy-card">
    <h3>일기 접근과 대화 기록</h3>
    <p role="status">{!prefs ? '설정 확인 중…' : prefs.consented ? '일기 사용에 동의했어요.' : '아직 동의하지 않았어요. 대화창에서 동의하면 시작할 수 있어요.'}</p>
    {error && <p className="sync-error" role="alert">{error}</p>}
    <div className="panel-actions">
      {prefs?.consented && <Button variant="outline" data-action="codex-revoke-consent" onClick={() => void change(async () => {
        if (!await askConfirm('동의를 거두면 Niki가 일기를 읽는 모든 기능이 멈춰요. 이미 저장된 대화는 그대로 남아요. 거둘까요?', { okLabel: '동의 거두기', danger: true })) return;
        return run<Prefs>('codex_set_consent', { consented: false });
      })}>동의 거두기</Button>}
    </div>
    <h4 className="codex-subhead">대화에 사용할 일기장</h4>
    <p className="subtle">체크를 끄면 해당 일기장을 AI에게 보내거나 검색하지 않아요.</p>
    <div className="codex-journal-list">{data?.journals.map(journal => <label className="codex-check codex-journal-choice" key={journal.id}>
      <input type="checkbox" disabled={!prefs} checked={!!prefs && !prefs.excluded_journals.includes(journal.id)}
        data-action={prefs?.excluded_journals.includes(journal.id) ? 'codex-unhide-journal' : 'codex-hide-journal'}
        onChange={event => { const excluded = !event.target.checked; void change(() => run<Prefs>('codex_set_journal_excluded', { journalId: journal.id, excluded })); }} />
      <span>{journal.name}</span>
    </label>)}</div>
    <h4 className="codex-subhead">대화 기록</h4>
    <p className="subtle">같은 Google Drive 계정으로 대화·요약·검색 데이터를 동기화해요.</p>
    <div className="panel-actions">
      <Button variant="outline" className="danger" data-action="codex-clear-chat" onClick={() => void change(async () => {
        if (!await askConfirm('대화와 대화 요약을 지울까요? 동기화된 다른 기기에서도 삭제됩니다. 일기는 그대로예요.', { okLabel: '대화 지우기', danger: true })) return;
        await run('codex_clear_conversation');
      })}>대화 기록 지우기</Button>
    </div>
  </div>;
}

function UsageCard({ connected }: { connected: boolean }) {
  const { run } = useApp();
  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [models, setModels] = useState<ModelChoice[]>([]);
  const [model, setModel] = useState('');
  const [fallback, setFallback] = useState(false);
  const [background, setBackground] = useState(true);
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  useEffect(() => {
    void run<Prefs>('codex_prefs').then(next => { setPrefs(next); setModel(next.model ?? ''); setFallback(next.model_fallback); setBackground(next.background); }).catch(() => undefined);
  }, [run]);
  const loadModels = async () => {
    setError('');
    try { setModels(await run<ModelChoice[]>('codex_models')); } catch (e) { setError(errorMessage(e)); }
  };
  const save = async () => {
    setError(''); setMessage('');
    try {
      const next = await run<Prefs>('codex_set_prefs', { model: model || null, modelFallback: fallback, background });
      setPrefs(next); setMessage('저장했어요.');
      window.dispatchEvent(new Event(CODEX_PREFS_CHANGED));
    } catch (e) { setError(errorMessage(e)); }
  };
  const options = models.some(item => item.id === model) || !model ? models : [{ id: model, name: model }, ...models];
  return <div className="panel-card settings-card" data-testid="usage-card">
    <h3>답변 방식</h3>
    <p className="subtle">내 ChatGPT 계정의 Codex 사용 한도가 적용돼요.</p>
    <label className="codex-field">답변에 사용할 AI
      <NativeSelect size="sm" aria-label="Niki가 쓸 모델" value={model} onChange={event => setModel(event.target.value)}>
        <NativeSelectOption value="">기본 ({prefs?.default_model ?? 'gpt-6-luna'})</NativeSelectOption>
        {options.map(item => <NativeSelectOption key={item.id} value={item.id}>{item.name}</NativeSelectOption>)}
      </NativeSelect>
    </label>
    <div className="panel-actions"><Button variant="outline" size="sm" disabled={!connected} data-action="codex-load-models" onClick={() => void loadModels()}>AI 목록 불러오기</Button></div>
    <label className="codex-check"><input type="checkbox" checked={fallback} onChange={event => setFallback(event.target.checked)} /><span>선택한 AI를 쓸 수 없으면 다른 AI로 답하기</span></label>
    <label className="codex-check"><input type="checkbox" checked={background} onChange={event => setBackground(event.target.checked)} /><span>지난 일기와 긴 대화 자동 요약</span></label>
    <p className="subtle">자동 요약을 끄면 AI 사용량을 줄일 수 있어요. 일기 검색은 계속 사용할 수 있어요.</p>
    {error && <p className="sync-error" role="alert">{error}</p>}
    {message && <p role="status">{message}</p>}
    <div className="panel-actions"><Button data-action="codex-save-prefs" onClick={() => void save()}>저장</Button></div>
  </div>;
}

function MemoryCard({ connected }: { connected: boolean }) {
  const { run } = useApp();
  const [info, setInfo] = useState<MemoryInfo | null>(null);
  const [error, setError] = useState('');
  const load = () => run<MemoryInfo>('codex_memory_info').then(setInfo).catch(e => setError(errorMessage(e)));
  useEffect(() => {
    void load();
    const timer = setInterval(() => { void load(); }, 3000);
    return () => clearInterval(timer);
  }, [run, connected]);
  const reset = async (what: 'index' | 'model' | 'summaries', warning: string) => {
    if (!await askConfirm(warning, { okLabel: '지우기', danger: true })) return;
    setError('');
    try { await run('codex_memory_reset', { what }); await load(); } catch (e) { setError(errorMessage(e)); }
  };
  return <div className="panel-card settings-card" data-testid="memory-card">
    <h3>일기 검색과 기억</h3>
    <p>검색 데이터와 요약은 기기 간 공유돼요. 검색 도구는 각 기기에서 한 번 내려받아요.</p>
    {info && <>
      <p role="status"><strong>검색 모델</strong> {info.model_installed ? `설치됨 (${megabytes(info.model_bytes)})` : info.downloading ? '내려받는 중…' : info.failed ? '내려받지 못했어요. 잠시 후 다시 시도해요.' : connected ? '곧 내려받아요' : 'GPT 계정을 연결하면 내려받아요'}</p>
      <p><strong>검색 준비</strong> 일기 {info.indexed_entries}/{info.total_entries}편 · {megabytes(info.index_bytes)}</p>
      <div className="codex-progress" role="progressbar" aria-label="일기 검색 준비" aria-valuemin={0} aria-valuemax={100} aria-valuenow={info.total_entries ? Math.round(info.indexed_entries / info.total_entries * 100) : 100}><span style={{ width: `${info.total_entries ? Math.round(info.indexed_entries / info.total_entries * 100) : 100}%` }} /></div>
      <p><strong>요약 기억</strong> 월별 요약 {info.months_summarized}개 · 대화 요약 {info.chat_summarized ? '있음' : '없음'}</p>
    </>}
    {error && <p className="sync-error" role="alert">{error}</p>}
    <div className="panel-actions">
      <Button variant="outline" size="sm" data-action="codex-reset-index" onClick={() => void reset('index', '검색 데이터를 지우고 다시 만들까요? 다른 기기에도 반영됩니다. 일기는 그대로예요.')}>검색 다시 준비하기</Button>
      <Button variant="outline" size="sm" data-action="codex-reset-model" onClick={() => void reset('model', '이 기기의 검색 도구를 다시 받을까요? 약 112MB를 내려받고 검색 데이터 초기화는 다른 기기에도 반영됩니다. 일기는 그대로예요.')}>검색 도구 다시 받기</Button>
      <Button variant="outline" size="sm" data-action="codex-reset-summaries" onClick={() => void reset('summaries', '월별·대화 요약을 모든 동기화 기기에서 지울까요? 대화와 일기는 남고, 요약은 필요할 때 다시 만들어요.')}>요약 기억 지우기</Button>
    </div>
  </div>;
}

export function CodexSettings() {
  const { run } = useApp();
  const chat = useChat();
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
    <div className="wide-head"><h2>AI 연결</h2></div>
    <div className="panel-card settings-card codex-settings">
      <h3>ChatGPT 계정</h3>
      <p>계정을 연결하면 니키와 대화하고 일기를 요약할 수 있어요.</p>
      {status?.connected ? <p role="status" className="codex-connected">연결됨 · {status.plan || 'ChatGPT'}</p> : <p role="status">{status ? status.installed ? 'ChatGPT 계정을 연결해 주세요.' : '연결 프로그램 Codex가 필요해요. 아래 설치 안내를 확인해 주세요.' : '연결 상태 확인 중…'}</p>}
      {error && <p className="sync-error" role="alert">{error}</p>}
      {login && <div className="codex-login">
        <p>브라우저에서 ChatGPT에 로그인한 뒤 아래 코드를 입력하세요.</p>
        <code className="codex-device-code">{login.user_code}</code>
        <p className="subtle">{login.verification_url}</p>
        <div className="panel-actions">
          <Button disabled={busy} onClick={() => void perform(async () => { await run('codex_open_login'); })}>로그인 페이지 열기</Button>
          <Button variant="outline" disabled={busy} onClick={() => void perform(() => navigator.clipboard.writeText(login.user_code))}>코드 복사</Button>
        </div>
        <p className="subtle">로그인을 마치면 자동으로 연결돼요.</p>
      </div>}
      <div className="panel-actions">
        {status?.connected && <Button onClick={() => { if (!chat.open) chat.toggle(); }}>대화 시작</Button>}
        {!status?.connected && !login && !status?.login_pending && <Button disabled={busy || !status?.installed} data-action="codex-connect" onClick={() => void perform(async () => { setLogin(await run<Login>('codex_connect')); })}>ChatGPT 계정으로 연결</Button>}
        {(status?.connected || login || status?.login_pending) && <Button variant="outline" disabled={busy} data-action="codex-disconnect" onClick={disconnect}>{status?.connected ? '연결 해제' : '로그인 취소'}</Button>}
        {!login && <Button variant="outline" disabled={busy} onClick={() => void perform(async () => setStatus(await run<CodexStatus>('codex_status')))}>상태 새로고침</Button>}
      </div>
    </div>
    {status && !status.installed && <div className="panel-card settings-card">
      <h3>연결 준비</h3>
      <p>Codex를 설치하고 ChatGPT 계정으로 로그인한 뒤, Niki에서 연결 상태를 다시 확인해 주세요.</p>
      <p>이미 설치했다면 고급 설정에서 프로그램 위치를 확인할 수 있어요.</p>
    </div>}
    <PrivacyCard />
    <details className="panel-card settings-card codex-details" data-testid="data-details">
      <summary>AI에 전달되는 정보</summary>
      <ul className="codex-data-list">
        <li>일기 요약: 현재 열린 일기의 제목과 본문을 OpenAI로 보내요.</li>
        <li>대화: 메시지, 최근 대화, 최근·관련 일기 원문, 오래된 일기 앞부분과 월별 요약을 보내요.</li>
        <li>자동 요약: 일기 앞부분과 긴 대화를 보내 요약해요.</li>
        <li>일기 검색은 이 기기에서 해요. 검색 도구 준비 전에는 메시지와 월별 요약으로 AI에게 검색을 요청해요.</li>
      </ul>
    </details>
    <details className="panel-card settings-card codex-details" data-testid="advanced-settings">
      <summary>고급 설정 <span>답변 방식 · 연결 프로그램 · 검색 관리</span></summary>
      <UsageCard connected={!!status?.connected} />
      <CliCard installed={status?.installed} />
      <MemoryCard connected={!!status?.connected} />
    </details>
  </>;
}
