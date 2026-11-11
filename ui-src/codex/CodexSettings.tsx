import { useEffect, useState } from 'react';
import { useApp } from '../app/AppContext';
import { Button } from '../components/ui/button';
import { errorMessage } from '../lib/format';
import { Input } from '../components/ui/input';
import { NativeSelect, NativeSelectOption } from '../components/ui/native-select';
import { askConfirm } from '../lib/dialog';
import { CODEX_PREFS_CHANGED, GPT_CONNECTION_CHANGED } from './useGptConnection';
import type { Prefs } from './ChatContext';

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
    <h3>Codex CLI</h3>
    {info?.path
      ? <>
        <p role="status"><strong>{info.version ? `버전 ${info.version}` : '버전을 확인하지 못했어요'}</strong>{info.supported ? ' · 사용할 수 있어요' : ` · ${info.min_version} 이상이 필요해요. Codex를 업데이트해 주세요.`}</p>
        <p className="subtle codex-path">{info.path}{info.custom ? ' (직접 지정)' : ' (자동 감지)'}</p>
      </>
      : <p role="status">Codex를 찾지 못했어요. 터미널에서 <code>codex --version</code>이 실행되는지 확인하거나, 설치된 파일의 전체 경로를 아래에 입력해 주세요. 설치 직후에는 Niki를 다시 켜면 인식되는 경우가 많아요.</p>}
    {error && <p className="sync-error" role="alert">{error}</p>}
    <details className="oauth-details">
      <summary>{info?.path ? '다른 Codex 파일 사용' : 'Codex 경로 직접 지정'}</summary>
      <div className="oauth-details-body">
        <Input aria-label="Codex 실행 파일 전체 경로" placeholder="/Users/me/.local/bin/codex" value={path} onChange={event => setPath(event.target.value)} />
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
  const hidden = (data?.journals ?? []).filter(journal => prefs?.excluded_journals.includes(journal.id));
  return <div className="panel-card settings-card" data-testid="privacy-card">
    <h3>개인정보와 동의</h3>
    <p role="status">{prefs?.consented ? 'Niki에게 일기를 보여 주는 것에 동의했어요.' : '아직 동의하지 않았어요. Niki 대화창에서 동의하면 시작할 수 있어요.'}</p>
    {error && <p className="sync-error" role="alert">{error}</p>}
    <div className="panel-actions">
      {prefs?.consented && <Button variant="outline" data-action="codex-revoke-consent" onClick={() => void change(async () => {
        if (!await askConfirm('동의를 거두면 Niki가 일기를 읽는 모든 기능이 멈춰요. 이미 저장된 대화는 그대로 남아요. 거둘까요?', { okLabel: '동의 거두기', danger: true })) return;
        return run<Prefs>('codex_set_consent', { consented: false });
      })}>동의 거두기</Button>}
    </div>
    <h4 className="codex-subhead">Niki에게 숨긴 일기장</h4>
    {hidden.length
      ? hidden.map(journal => <div className="template-row" key={journal.id}><span>{journal.name}</span>
        <Button variant="outline" size="sm" data-action="codex-unhide-journal" onClick={() => void change(() => run<Prefs>('codex_set_journal_excluded', { journalId: journal.id, excluded: false }))}>다시 보이기</Button></div>)
      : <p className="subtle">숨긴 일기장이 없어요. 일기장 목록에서 우클릭하면 'Niki에게 숨기기'를 고를 수 있어요. 숨긴 일기장은 전송, 검색, 색인에서 모두 빠져요.</p>}
    <h4 className="codex-subhead">대화 기록</h4>
    <p className="subtle">대화와 요약은 이 기기에만 저장되고 Google Drive로 동기화되지 않아요. 다른 기기에서는 새 대화로 시작해요.</p>
    <div className="panel-actions">
      <Button variant="outline" className="danger" data-action="codex-clear-chat" onClick={() => void change(async () => {
        if (!await askConfirm('지금까지의 모든 대화와 대화 요약을 이 기기에서 지울까요? 일기는 지워지지 않아요.', { okLabel: '대화 지우기', danger: true })) return;
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
    <h3>모델과 사용량</h3>
    <p className="subtle">Niki의 답변은 내 ChatGPT 계정의 Codex 사용 한도로 처리돼요.</p>
    <label className="codex-field">모델
      <NativeSelect size="sm" aria-label="Niki가 쓸 모델" value={model} onChange={event => setModel(event.target.value)}>
        <NativeSelectOption value="">기본 ({prefs?.default_model ?? 'gpt-6-luna'})</NativeSelectOption>
        {options.map(item => <NativeSelectOption key={item.id} value={item.id}>{item.name}</NativeSelectOption>)}
      </NativeSelect>
    </label>
    <div className="panel-actions"><Button variant="outline" size="sm" disabled={!connected} data-action="codex-load-models" onClick={() => void loadModels()}>사용할 수 있는 모델 불러오기</Button></div>
    <label className="codex-check"><input type="checkbox" checked={fallback} onChange={event => setFallback(event.target.checked)} /> 선택한 모델을 쓸 수 없으면 다른 모델로 대신 답하기</label>
    <p className="subtle">끄면 모델이 막혔을 때 대화가 멈추고 안내가 나와요. 켜면 계정에서 쓸 수 있는 다른 모델로 이어서 답해요.</p>
    <label className="codex-check"><input type="checkbox" checked={background} onChange={event => setBackground(event.target.checked)} /> 백그라운드 정리 사용 (월별 요약, 긴 대화 요약)</label>
    <p className="subtle">끄면 월별 요약과 긴 대화 요약을 만들지 않아 Codex 사용량을 아낄 수 있어요. 일기 검색용 색인은 이 기기에서 계속 만들어요.</p>
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
    {info && <>
      <p role="status"><strong>검색 모델</strong> {info.model_installed ? `설치됨 (${megabytes(info.model_bytes)})` : info.downloading ? '내려받는 중…' : info.failed ? '내려받지 못했어요. 잠시 후 다시 시도해요.' : connected ? '곧 내려받아요' : 'GPT 계정을 연결하면 내려받아요'}</p>
      <p><strong>검색 색인</strong> 일기 {info.indexed_entries}/{info.total_entries}편 · {megabytes(info.index_bytes)}</p>
      <div className="codex-progress" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={info.total_entries ? Math.round(info.indexed_entries / info.total_entries * 100) : 100}><span style={{ width: `${info.total_entries ? Math.round(info.indexed_entries / info.total_entries * 100) : 100}%` }} /></div>
      <p><strong>요약 기억</strong> 월별 요약 {info.months_summarized}개 · 대화 요약 {info.chat_summarized ? '있음' : '없음'}</p>
    </>}
    {error && <p className="sync-error" role="alert">{error}</p>}
    <div className="panel-actions">
      <Button variant="outline" size="sm" data-action="codex-reset-index" onClick={() => void reset('index', '검색 색인을 지우고 다시 만들까요? 일기는 그대로예요. 다시 만드는 동안 검색은 단어로만 찾을 수 있어요.')}>색인 다시 만들기</Button>
      <Button variant="outline" size="sm" data-action="codex-reset-model" onClick={() => void reset('model', '검색 모델과 색인을 지우고 다시 받을까요? 약 112MB를 다시 내려받아요.')}>검색 모델 삭제 후 다시 받기</Button>
      <Button variant="outline" size="sm" data-action="codex-reset-summaries" onClick={() => void reset('summaries', '월별 요약과 대화 요약을 지울까요? 대화 내용 자체는 그대로 남고, 요약은 필요할 때 다시 만들어져요.')}>요약 기억 지우기</Button>
    </div>
  </div>;
}

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
    <CliCard installed={status?.installed} />
    <PrivacyCard />
    <UsageCard connected={!!status?.connected} />
    <MemoryCard connected={!!status?.connected} />
    <div className="panel-card settings-card">
      <h3>전송되는 정보</h3>
      <p>요약할 일기, 그리고 Niki와 대화할 때 찾은 일기의 제목과 본문이 OpenAI에 전송됩니다.</p>
      <p>요약 버튼을 누르면 현재 일기를 전송합니다. 메시지를 보내면 입력한 내용, 최근 대화, 그리고 최근 일기와 메시지와 관련된 일기의 원문, 오래된 일기의 앞부분, 월별 개요가 함께 전송됩니다. 관련 일기는 이 기기 안에서 찾습니다. GPT 계정을 연결하면 앱이 일기 검색 모델(약 112MB)을 한 번 내려받아 의미가 비슷한 일기도 이 기기 안에서 찾으며, 이때 일기는 검색을 위해 외부로 보내지 않습니다. 검색 모델이 준비되기 전에는 관련 일기를 찾기 위해 메시지와 월별 개요를 담은 짧은 요청을 한 번 더 보냅니다. 월별 개요와 긴 대화의 요약을 만들 때도 해당 일기 앞부분과 대화가 전송되며, 만들어진 요약과 검색 색인은 이 기기에만 저장됩니다. 대화 기록은 이 기기에 저장되어 모든 일기장에서 공통으로 사용합니다.</p>
    </div>
  </>;
}
