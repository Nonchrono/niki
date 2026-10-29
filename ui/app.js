const invoke = (name, args = {}) => window.__TAURI__.core.invoke(name, args);
const app = document.getElementById('app');
const toastBox = document.getElementById('toast');
const importFile = document.getElementById('import-file');
const attachFile = document.getElementById('attach-file');
const GOOGLE_ICON = '<svg class="google-icon" viewBox="0 0 24 24" aria-hidden="true"><path fill="#4285F4" d="M23.49 12.27c0-.79-.07-1.54-.19-2.27H12v4.51h6.47c-.29 1.48-1.14 2.73-2.4 3.58v3h3.86c2.26-2.09 3.56-5.17 3.56-8.82z"/><path fill="#34A853" d="M12 24c3.24 0 5.95-1.08 7.93-2.91l-3.86-3c-1.08.72-2.45 1.16-4.07 1.16-3.13 0-5.78-2.11-6.73-4.96H1.29v3.09C3.26 21.3 7.31 24 12 24z"/><path fill="#FBBC05" d="M5.27 14.29c-.25-.72-.38-1.49-.38-2.29s.13-1.57.38-2.29V6.62H1.29C.47 8.24 0 10.06 0 12s.47 3.76 1.29 5.38l3.98-3.09z"/><path fill="#EA4335" d="M12 4.75c1.77 0 3.35.61 4.6 1.8l3.42-3.42C17.95 1.19 15.24 0 12 0 7.31 0 3.26 2.7 1.29 6.62l3.98 3.09c.95-2.85 3.6-4.96 6.73-4.96z"/></svg>';
const mdi = path => '<svg class="mdi-icon" viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="'+path+'"/></svg>';
const MDI = {
  collapse: mdi('M18.41,7.41L17,6L11,12L17,18L18.41,16.59L13.83,12L18.41,7.41M12.41,7.41L11,6L5,12L11,18L12.41,16.59L7.83,12L12.41,7.41Z'),
  expand: mdi('M5.59,7.41L7,6L13,12L7,18L5.59,16.59L10.17,12L5.59,7.41M11.59,7.41L13,6L19,12L13,18L11.59,16.59L16.17,12L11.59,7.41Z'),
  all: mdi('M16,15H9V13H16V15M19,11H9V9H19V11M19,7H9V5H19V7M3,5V21H19V23H3A2,2 0 0,1 1,21V5H3M21,1A2,2 0 0,1 23,3V17C23,18.11 22.11,19 21,19H7A2,2 0 0,1 5,17V3C5,1.89 5.89,1 7,1H21M7,3V17H21V3H7Z'),
  calendar: mdi('M7,11H9V13H7V11M21,5V19A2,2 0 0,1 19,21H5C3.89,21 3,20.1 3,19V5A2,2 0 0,1 5,3H6V1H8V3H16V1H18V3H19A2,2 0 0,1 21,5M5,7H19V5H5V7M19,19V9H5V19H19M15,13V11H17V13H15M11,13V11H13V13H11M7,15H9V17H7V15M15,17V15H17V17H15M11,17V15H13V17H11Z'),
  media: mdi('M21,17H7V3H21M21,1H7A2,2 0 0,0 5,3V17A2,2 0 0,0 7,19H21A2,2 0 0,0 23,17V3A2,2 0 0,0 21,1M3,5H1V21A2,2 0 0,0 3,23H19V21H3M15.96,10.29L13.21,13.83L11.25,11.47L8.5,15H19.5L15.96,10.29Z'),
  memories: mdi('M12,1L9,9L1,12L9,15L12,23L15,15L23,12L15,9L12,1M12,6.59L13.42,10.37L13.63,10.58L17.41,12L13.63,13.42L13.42,13.63L12,17.41L10.58,13.63L10.37,13.42L6.59,12L10.37,10.58L10.58,10.37L12,6.59Z'),
  plus: mdi('M19,13H13V19H11V13H5V11H11V5H13V11H19V13Z'),
  trash: mdi('M9,3V4H4V6H5V19A2,2 0 0,0 7,21H17A2,2 0 0,0 19,19V6H20V4H15V3H9M7,6H17V19H7V6M9,8V17H11V8H9M13,8V17H15V8H13Z'),
  settings: mdi('M12,15.5A3.5,3.5 0 0,1 8.5,12A3.5,3.5 0 0,1 12,8.5A3.5,3.5 0 0,1 15.5,12A3.5,3.5 0 0,1 12,15.5M19.43,12.97C19.47,12.65 19.5,12.33 19.5,12C19.5,11.67 19.47,11.34 19.43,11L21.54,9.37C21.73,9.22 21.78,8.95 21.66,8.73L19.66,5.27C19.54,5.05 19.27,4.96 19.05,5.05L16.56,6.05C16.04,5.66 15.5,5.32 14.87,5.07L14.5,2.42C14.46,2.18 14.25,2 14,2H10C9.75,2 9.54,2.18 9.5,2.42L9.13,5.07C8.5,5.32 7.96,5.66 7.44,6.05L4.95,5.05C4.73,4.96 4.46,5.05 4.34,5.27L2.34,8.73C2.21,8.95 2.27,9.22 2.46,9.37L4.57,11C4.53,11.34 4.5,11.67 4.5,12C4.5,12.33 4.53,12.65 4.57,12.97L2.46,14.63C2.27,14.78 2.21,15.05 2.34,15.27L4.34,18.73C4.46,18.95 4.73,19.03 4.95,18.95L7.44,17.94C7.96,18.34 8.5,18.68 9.13,18.93L9.5,21.58C9.54,21.82 9.75,22 10,22H14C14.25,22 14.46,21.82 14.5,21.58L14.87,18.93C15.5,18.67 16.04,18.34 16.56,17.94L19.05,18.95C19.27,19.03 19.54,18.95 19.66,18.73L21.66,15.27C21.78,15.05 21.73,14.78 21.54,14.63L19.43,12.97Z')
};
let data = {locked:false,lock_enabled:false,app_version:'0.1.0',updater_config:{endpoint:'',pubkey:'',auto_check:true,skipped_version:''},journals:[],entries:[],templates:[],sync_conflicts:[],pending_resolution_count:0,sync_status:{},synced_entry_ids:[]};
let journal = 'all', entryId = null, view = 'timeline', search = '', tag = '', favorites = false, dateFilter = '';
let month = new Date(new Date().getFullYear(),new Date().getMonth(),1);
let preview = false, saveTimer = null, saving = false, saveAgain = false, visibleSyncKey = '', settingsTab = 'sync', sidebarCollapsed = false;
let updateInfo = null, updateDismissed = false, generatedKeyPair = null, generatedManifest = '';
const media = new Map();
const esc = value => String(value ?? '').replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;').replaceAll("'",'&#39;');
const localDate = () => { const d=new Date(); return new Date(d.getTime()-d.getTimezoneOffset()*60000).toISOString().slice(0,10); };
const journalName = id => data.journals.find(j => j.id === id)?.name || '일지';
const active = () => data.entries.filter(e => !e.deleted_at);
const selected = () => data.entries.find(e => e.id === entryId);
const dateLabel = value => new Date(value+'T12:00:00').toLocaleDateString('ko-KR',{year:'numeric',month:'long',day:'numeric'});
const timeLabel = value => value ? new Date(value).toLocaleString('ko-KR') : '아직 동기화하지 않음';
function accountLabel(s = data.sync_status || {}) {
  if(!s.connected) return '연결되지 않음';
  if(s.account_name && s.account_email) return s.account_name + ' (' + s.account_email + ')';
  return s.account_email || s.account_name || 'Google 계정 연결됨';
}
function toast(message, bad=false) { toastBox.textContent=String(message); toastBox.className=bad?'show error':'show'; setTimeout(()=>toastBox.className='',4200); }
function fail(e) { console.error(e); toast(e,true); }
async function reload() { data=await invoke('bootstrap'); if(data.locked){visibleSyncKey='';media.clear();document.querySelectorAll('.media-overlay,.context-menu').forEach(node=>node.remove());} }
function closeContextMenu() { document.querySelectorAll('.context-menu').forEach(node=>node.remove()); }
async function autoCheckUpdate() {
  const cfg = data.updater_config;
  if(data.locked || !cfg?.auto_check || !cfg?.endpoint || cfg.endpoint.includes('YOUR_GITHUB_ID')) return;
  try {
    const info = await invoke('updater_check');
    updateInfo = info;
    if(info.update_available && info.latest_version !== cfg.skipped_version) {
      updateDismissed = false;
      render();
      toast('새 버전(v'+info.latest_version+') 업데이트가 있습니다!');
    }
  } catch(e) {
    console.warn('Auto update check skipped:', e);
  }
}
async function autoSyncDrive() {
  if(data.locked || !data.sync_status?.connected || data.sync_status?.sync_paused) return;
  try {
    await invoke('drive_sync_now');
    await reload();
    render();
  } catch(e) {
    console.warn('Auto drive sync skipped:', e);
  }
}
async function startGoogleLogin(clientId='', clientSecret='', btn=null) {
  if(btn) btn.disabled = true;
  try {
    toast('브라우저에서 로그인할 Google 계정을 선택하세요…');
    await invoke('drive_connect', {clientId, clientSecret});
    await reload();
    render();
    const who = data.sync_status?.account_email || data.sync_status?.account_name || 'Google 계정';
    toast(who + ' 연결 완료! Google Drive와 자동 동기화 중…');
    try {
      const res = await invoke('drive_sync_now');
      await reload();
      render();
      toast(who + ' 동기화 완료 (업로드 '+res.uploaded_revisions+'건, 다운로드 '+res.downloaded_revisions+'건)');
    } catch(syncErr) {
      await reload();
      render();
      toast(who + ' 연결됨 · 동기화 안내: ' + syncErr, true);
    }
  } finally {
    if(btn) btn.disabled = false;
  }
}
function side(action,icon,label,count='',id='',current=false) { return '<button class="side-row '+(current?'active':'')+'" data-action="'+action+'" data-id="'+esc(id)+'" title="'+esc(label)+'"><span class="icon">'+icon+'</span><span class="side-label">'+esc(label)+'</span><span class="count">'+count+'</span></button>'; }
function sidebar() {
  const greeting = data.nickname?.trim() || '안녕하세요!';
  let out='<aside class="sidebar"><div class="brand"><span class="brand-text" title="'+esc(greeting)+'">'+esc(greeting)+'</span><button class="sidebar-toggle" data-action="toggle-sidebar" title="'+(sidebarCollapsed?'메뉴바 열기':'메뉴바 접기')+'" aria-label="'+(sidebarCollapsed?'메뉴바 열기':'메뉴바 접기')+'">'+(sidebarCollapsed?MDI.expand:MDI.collapse)+'</button></div><div class="side-caption">둘러보기</div>';
  out+=side('all',MDI.all,'전체 기록',active().length,'',view==='timeline'&&journal==='all');
  out+=side('calendar',MDI.calendar,'달력','', '',view==='calendar')+side('media',MDI.media,'미디어','', '',view==='media')+side('memories',MDI.memories,'이 날의 기록','', '',view==='memories');
  out+='<div class="side-caption">일기장 목록</div>';
  for(const j of data.journals) out+='<button class="side-row '+(view==='timeline'&&journal===j.id?'active':'')+'" data-action="journal" data-id="'+esc(j.id)+'" title="'+esc(j.name)+' (우클릭하여 수정·삭제)"><span class="dot" style="background:'+esc(j.color)+'"></span><span class="side-label">'+esc(j.name)+'</span><span class="count">'+active().filter(e=>e.journal_id===j.id).length+'</span></button>';
  out+='<button class="side-row side-add" data-action="new-journal" title="새 일지"><span class="icon">'+MDI.plus+'</span><span class="side-label">새 일지</span></button><div class="side-bottom">';
  out+=side('trash',MDI.trash,'휴지통',data.entries.filter(e=>e.deleted_at).length,'',view==='trash')+side('settings',MDI.settings,'설정',data.sync_conflicts?.length?'⚠ '+data.sync_conflicts.length:'', '',view==='settings');
  return out+'</div></aside>';
}
function render() {
  if(data.locked) { app.innerHTML='<div class="lock-page"><div class="lock-card"><img class="brand-mark" src="./icon.svg" alt="" style="margin:auto"><h2>일기를 다시 열어 주세요</h2><p>잠금 암호를 입력하세요.</p><input id="unlock-password" type="password" placeholder="암호"><button class="primary" data-action="unlock">잠금 해제</button></div></div>'; document.getElementById('unlock-password').focus(); return; }
  const title=view==='timeline'?(journal==='all'?'전체 기록':journalName(journal)):({calendar:'달력',media:'미디어',memories:'이 날의 기록',trash:'휴지통',settings:'설정'}[view]);
  const s = data.sync_status || {};
  const syncBtn = s.connected
    ? '<button class="ghost" data-action="drive-sync" title="'+esc(accountLabel(s))+'">↻ 동기화'+(s.pending_upload_count?' ('+s.pending_upload_count+')':'')+'</button>'
    : '';
  const showUpdateBanner = updateInfo?.update_available && !updateDismissed && updateInfo.latest_version !== data.updater_config?.skipped_version;
  const updateBanner = showUpdateBanner
    ? '<div class="update-banner"><span>✨ <strong>새 버전(v'+esc(updateInfo.latest_version)+')</strong> 업데이트가 준비되었습니다.</span>'
      +(updateInfo.notes?'<span class="update-banner-notes" title="'+esc(updateInfo.notes)+'">'+esc(updateInfo.notes)+'</span>':'')
      +'<div class="update-banner-actions"><button class="primary" data-action="updater-install">지금 다운로드 및 재시작</button><button class="ghost" data-action="updater-dismiss">나중에</button><button class="ghost" data-action="updater-skip">이 버전 건너뛰기</button></div></div>'
    : '';
  app.innerHTML='<div class="shell'+(sidebarCollapsed?' collapsed':'')+'">'+sidebar()+'<main class="workspace"><header class="topbar"><div class="top-title">'+esc(title)+'</div><div class="top-spacer"></div>'+(view==='timeline'?'<label class="search-wrap"><span>⌕</span><input id="search" class="search" placeholder="기록, 태그, 장소 검색" value="'+esc(search)+'"></label>':'')+syncBtn+'<button class="primary" data-action="new-entry">＋ 새 기록</button></header>'+updateBanner+'<div class="content" id="content"></div></main></div>';
  renderContent();
}
function filtered() { const q=search.toLocaleLowerCase().trim(); return active().filter(e=>(journal==='all'||e.journal_id===journal)&&(!dateFilter||e.date===dateFilter)&&(!favorites||e.favorite)&&(!tag||e.tags.includes(tag))&&(!q||[e.title,e.body,e.location,e.weather,...e.tags].join(' ').toLocaleLowerCase().includes(q))).sort((a,b)=>b.date.localeCompare(a.date)||b.updated_at.localeCompare(a.updated_at)); }
function renderContent() {
  const c=document.getElementById('content');
  if(view==='timeline') timeline(c);
  else if(view==='calendar') calendar(c);
  else if(view==='media') mediaView(c);
  else if(view==='memories') memories(c);
  else if(view==='trash') trash(c);
  else settings(c);
}
function entrySyncBadge(id) {
  const s = data.sync_status;
  if(!s?.connected) return ' · 로컬 저장';
  if(data.sync_conflicts?.some(c => c.entry_id === id)) return ' · ⚠ 동기화 충돌';
  if(s.sync_paused) return ' · ⏸ 동기화 일시 중지됨';
  if(data.synced_entry_ids?.includes(id)) return ' · ☁ Drive 동기화됨';
  return ' · ↻ 동기화 대기 중';
}
function renderSyncSettings(pane) {
  const s = data.sync_status || {};
  const mb = ((s.synced_media_bytes || 0) / (1024 * 1024)).toFixed(2);
  pane.innerHTML='<div class="wide-head"><h2>Google Drive 동기화</h2></div>';

  const accountCard = document.createElement('div');
  accountCard.className = 'panel-card';
  if (s.connected) {
    const initial = (s.account_name || s.account_email || 'G').trim().charAt(0).toUpperCase();
    accountCard.innerHTML = '<h3>Google 계정</h3>'
      +'<div class="account-box">'
      +'<div class="account-avatar">'+esc(initial)+'</div>'
      +'<div class="account-info">'
      +'<div class="account-name">'+esc(s.account_name || 'Google 계정 연결됨')+' <span class="badge-connected">● 연결됨</span></div>'
      +'<div class="account-sub">'+esc(s.account_email || s.client_id || '')+'</div>'
      +'</div>'
      +'</div>'
      +'<div class="panel-actions">'
      +'<button class="primary" data-action="drive-sync" '+(s.sync_paused?'disabled':'')+'>↻ 지금 동기화</button>'
      +'<button class="ghost" data-action="google-login">다른 Google 계정으로 전환</button>'
      +'<button class="ghost danger" data-action="drive-disconnect">로그아웃</button>'
      +'</div>';
  } else {
    accountCard.innerHTML = '<h3>Google 계정으로 로그인</h3>'
      +'<div class="google-login-row">'
      +'<button class="google-btn" data-action="google-login">'+GOOGLE_ICON+'<span>Google 계정으로 로그인</span></button>'
      +'</div>'
      +(s.has_oauth_client ? '' : '<details class="oauth-details" open>'
        +'<summary>OAuth 클라이언트 설정 (최초 1회 입력 또는 oauth-client.json 저장)</summary>'
        +'<div class="oauth-details-body">'
        +'<p style="margin:0 0 8px">프로젝트 폴더의 <code>oauth-client.json</code> 파일에 클라이언트 ID와 보안 비밀을 저장한 뒤 빌드하면 이 입력창이 완전히 사라지며, 다른 사용자도 버튼만 눌러 바로 로그인할 수 있습니다.</p>'
        +'<input id="drive-client-id" placeholder="Google OAuth 데스크톱 클라이언트 ID (예: 123456-abc.apps.googleusercontent.com)" value="'+esc(s.client_id||'')+'">'
        +'<input id="drive-client-secret" type="password" placeholder="클라이언트 보안 비밀 (client_secret)">'
        +'</div>'
        +'</details>');
  }
  pane.appendChild(accountCard);
  if (!s.connected) return;

  const syncCard=document.createElement('div');
  syncCard.className='panel-card';
  syncCard.innerHTML='<h3>동기화 상태</h3>'
    +'<p><strong>계정 상태:</strong> '+esc(accountLabel(s))
    +' · <strong>동기화:</strong> '+(s.sync_paused ? '일시 중지됨' : '자동 동기화 활성')+'</p>'
    +'<p><strong>마지막 동기화:</strong> '+esc(timeLabel(s.last_sync_at))
    +' · <strong>전송 대기:</strong> '+(s.pending_upload_count||0)+'건'
    +' · <strong>충돌:</strong> '+(s.conflict_count||0)+'건'
    +' · <strong>원격 버전:</strong> '+(s.remote_object_count||0)+'개'
    +' · <strong>동기화된 첨부:</strong> '+mb+' MB</p>'
    +(s.last_sync_error ? '<p style="color:#b42318"><strong>최근 오류:</strong> '+esc(s.last_sync_error)+'</p>' : '')
    +(data.pending_resolution_count ? '<p>충돌 해결 '+data.pending_resolution_count+'건이 다음 동기화 시 업로드됩니다.</p>' : '')
    +'<div class="panel-actions">'
    +'<button class="primary" data-action="drive-sync" '+(!s.connected||s.sync_paused?'disabled':'')+'>지금 동기화</button>'
    +'<button class="ghost" data-action="drive-toggle-pause">'+(s.sync_paused?'동기화 재개':'동기화 일시 중지')+'</button>'
    +'</div>';
  pane.appendChild(syncCard);

  if(data.sync_conflicts?.length){
    const conflicts=document.createElement('div');
    conflicts.className='panel-card';
    conflicts.innerHTML='<h3>동기화 충돌 ('+data.sync_conflicts.length+'건)</h3><p>같은 기록이 두 기기에서 동시에 수정되었습니다. 각 버전을 확인한 뒤 유지할 기록을 선택하세요.</p>'
      +data.sync_conflicts.map(c=>'<div class="panel-card"><p><strong>이 기기:</strong> '+esc(c.local&&!c.local.deleted_at?(c.local.title||'제목 없음')+' — '+c.local.body:'삭제됨')+'</p><p><strong>다른 기기:</strong> '+esc(c.remote&&!c.remote.deleted_at?(c.remote.title||'제목 없음')+' — '+c.remote.body:'삭제됨')+'</p><button class="ghost" data-action="resolve-conflict" data-choice="local" data-id="'+esc(c.id)+'">이 기기 기록 유지</button> <button class="ghost" data-action="resolve-conflict" data-choice="remote" data-id="'+esc(c.id)+'">다른 기기 기록 사용</button></div>').join('');
    pane.appendChild(conflicts);
  }
}
function timeline(c=document.getElementById('content')) {
  const entries=filtered(), tags=[...new Set(active().flatMap(e=>e.tags))].sort();
  c.innerHTML='<section class="list-pane"><div class="list-head"><h2>'+esc(dateFilter?dateLabel(dateFilter):journal==='all'?'모든 기록':journalName(journal))+'</h2><div class="subtle">'+entries.length+'개의 기록 '+(dateFilter?'<a href="#" data-action="clear-date">날짜 필터 지우기</a>':'')+'</div></div><div class="list-controls"><button class="chip '+(favorites?'active':'')+'" data-action="favorites">♥ 즐겨찾기</button><select class="filter-select" id="tag-filter"><option value="">모든 태그</option>'+tags.map(t=>'<option value="'+esc(t)+'" '+(tag===t?'selected':'')+'># '+esc(t)+'</option>').join('')+'</select></div><div class="entry-list">'+(entries.length?entries.map(e=>'<button class="entry-card '+(entryId===e.id?'active':'')+'" data-action="entry" data-id="'+esc(e.id)+'"><div class="entry-date">'+esc(dateLabel(e.date))+(journal==='all'?' · '+esc(journalName(e.journal_id)):'')+'</div><h3>'+esc(e.title||'제목 없는 기록')+'</h3><p>'+esc(e.body||'내용을 적어 보세요')+'</p><div class="entry-meta">'+(e.favorite?'<span>♥</span>':'')+(e.attachments.length?'<span>▧ '+e.attachments.length+'</span>':'')+(data.sync_conflicts?.some(x=>x.entry_id===e.id)?'<span>⚠ 충돌</span>':data.synced_entry_ids?.includes(e.id)?'<span>☁</span>':'')+e.tags.slice(0,2).map(t=>'<span>#'+esc(t)+'</span>').join('')+'</div></button>').join(''):'<div class="empty"><p>조건에 맞는 기록이 없습니다.</p></div>')+'</div></section><section class="editor-pane" id="editor-pane"></section>';
  editor();
}
function md(text) { return esc(text).split('\n').map(line=>{ let v=line.replace(/\*\*(.+?)\*\*/g,'<strong>$1</strong>').replace(/\*(.+?)\*/g,'<em>$1</em>'); if(v.startsWith('## ')) return '<h2>'+v.slice(3)+'</h2>'; if(v.startsWith('# ')) return '<h1>'+v.slice(2)+'</h1>'; if(v.startsWith('&gt; ')) return '<blockquote>'+v.slice(5)+'</blockquote>'; if(v.startsWith('- [ ] ')) return '<p>☐ '+v.slice(6)+'</p>'; if(v.startsWith('- [x] ')) return '<p>☑ '+v.slice(6)+'</p>'; if(v.startsWith('- ')) return '<p>• '+v.slice(2)+'</p>'; return '<p>'+(v||'<br>')+'</p>'; }).join(''); }
function editor() {
  const pane=document.getElementById('editor-pane'), e=selected();
  if(!pane) return;
  if(!e||e.deleted_at) { pane.innerHTML='<div class="empty" style="height:100%"><div class="empty-art">✎</div><h2>오늘을 기록해 보세요</h2><p>작은 순간도 시간이 지나면 소중한 이야기가 됩니다.</p><button class="primary" data-action="new-entry">＋ 새 기록 쓰기</button></div>'; return; }
  pane.innerHTML='<div class="editor-inner"><div class="editor-top"><span class="status" id="save-status">모든 변경 사항 저장됨'+esc(entrySyncBadge(e.id))+'</span><span class="spacer"></span><button class="icon-button" data-action="favorite-entry" title="즐겨찾기">'+(e.favorite?'♥':'♡')+'</button><button class="ghost" data-action="print">인쇄 / PDF</button><button class="icon-button" data-action="trash-entry" title="삭제">⋯</button></div><input class="editor-title" id="entry-title" placeholder="제목 없는 기록" value="'+esc(e.title)+'"><div class="editor-meta"><span>◷ <input id="entry-date" type="date" value="'+esc(e.date)+'"></span><span>◈ <select id="entry-journal">'+data.journals.map(j=>'<option value="'+esc(j.id)+'" '+(j.id===e.journal_id?'selected':'')+'>'+esc(j.name)+'</option>').join('')+'</select></span><span>⌗ <input id="entry-tags" placeholder="태그, 쉼표 구분" value="'+esc(e.tags.join(', '))+'"></span></div><div class="toolbar">'+[['bold','B'],['italic','I'],['heading','H'],['quote','❞'],['list','☷'],['check','☑']].map(x=>'<button class="tool" data-action="format" data-format="'+x[0]+'">'+x[1]+'</button>').join('')+'<span class="tool sep"></span><button class="tool" data-action="attach" title="첨부">📎</button><select id="insert-template" class="filter-select"><option value="">템플릿</option>'+data.templates.map(t=>'<option value="'+esc(t.id)+'">'+esc(t.name)+'</option>').join('')+'</select><span style="flex:1"></span><button class="chip '+(preview?'active':'')+'" data-action="preview">'+(preview?'편집':'미리보기')+'</button></div>'+(preview?'<div class="preview">'+md(e.body)+'</div>':'<textarea id="entry-body" class="editor-body" placeholder="오늘의 이야기를 시작해 보세요...">'+esc(e.body)+'</textarea>')+'<div class="attach-grid">'+e.attachments.map(a=>'<div class="attach-tile" id="att-'+esc(a.id)+'"><span>'+esc(a.name)+'</span><button data-action="remove-attachment" data-id="'+esc(a.id)+'" title="삭제">×</button></div>').join('')+'</div><div class="extra-fields"><label>장소<input id="entry-location" placeholder="장소" value="'+esc(e.location)+'"></label><label>날씨 / 메모<input id="entry-weather" placeholder="날씨" value="'+esc(e.weather)+'"></label></div></div>';
  e.attachments.filter(a=>a.mime.startsWith('image/')).forEach(async a=>{try{const url=await getMedia(a.id), box=document.getElementById('att-'+a.id); if(box){const img=document.createElement('img');img.src=url;img.alt=a.name;box.prepend(img);}}catch(x){console.warn(x);}});
}
async function getMedia(id) { if(data.locked)throw new Error('앱이 잠겨 있습니다'); if(!media.has(id)){ const x=await invoke('get_attachment',{attachmentId:id}); if(data.locked)throw new Error('앱이 잠겨 있습니다'); if(!/^(image\/(png|jpeg|gif|webp)|video\/(mp4|quicktime)|audio\/(mp4|mpeg|wav)|application\/(pdf|octet-stream))$/.test(x.mime))throw new Error('허용되지 않은 미디어 유형'); media.set(id,'data:'+x.mime+';base64,'+x.data); } return media.get(id); }
function calendar(c) { const y=month.getFullYear(),m=month.getMonth(),offset=new Date(y,m,1).getDay(), rec=active().filter(e=>journal==='all'||e.journal_id===journal); let out='<div class="wide-view"><div class="wide-head"><h2>'+y+'년 '+(m+1)+'월</h2><span class="spacer"></span><button class="ghost" data-action="prev-month">←</button><button class="ghost" data-action="today-month">오늘</button><button class="ghost" data-action="next-month">→</button></div><div class="calendar">'+['일','월','화','수','목','금','토'].map(x=>'<div class="weekday">'+x+'</div>').join(''); for(let i=0;i<42;i++){const d=new Date(y,m,i-offset+1),key=d.getFullYear()+'-'+String(d.getMonth()+1).padStart(2,'0')+'-'+String(d.getDate()).padStart(2,'0'),items=rec.filter(e=>e.date===key); out+='<button class="day '+(d.getMonth()===m?'':'outside')+' '+(key===localDate()?'today':'')+'" data-action="calendar-day" data-date="'+key+'"><span>'+d.getDate()+'</span>'+items.slice(0,3).map(e=>'<em>● '+esc(e.title||'제목 없는 기록')+'</em>').join('')+'</button>'; } c.innerHTML=out+'</div></div>'; }
function mediaView(c) { const items=active().filter(e=>journal==='all'||e.journal_id===journal).flatMap(e=>e.attachments.map(a=>({e,a}))); c.innerHTML='<div class="wide-view"><div class="wide-head"><h2>미디어</h2><span class="subtle">'+items.length+'개</span></div><div class="media-grid">'+items.map(({e,a})=>'<button class="media-card" data-action="entry" data-id="'+esc(e.id)+'"><div class="media-thumb" id="media-'+esc(a.id)+'">'+(a.mime.startsWith('video/')?'▶':a.mime.startsWith('audio/')?'♫':'▧')+'</div><span>'+esc(a.name)+'</span></button>').join('')+'</div></div>'; items.filter(x=>x.a.mime.startsWith('image/')).forEach(async ({a})=>{try{const url=await getMedia(a.id),box=document.getElementById('media-'+a.id); if(box){const img=document.createElement('img');img.src=url;img.alt=a.name;box.replaceChildren(img);}}catch(x){console.warn(x);}}); }
function memories(c) { const d=new Date(),suffix=String(d.getMonth()+1).padStart(2,'0')+'-'+String(d.getDate()).padStart(2,'0'),items=active().filter(e=>e.date.endsWith(suffix)&&e.date!==localDate()&&(journal==='all'||e.journal_id===journal)); c.innerHTML='<div class="wide-view"><div class="wide-head"><h2>이 날의 기록</h2></div>'+(items.length?items.map(e=>'<div class="panel-card"><span class="subtle">'+esc(dateLabel(e.date))+'</span><h3>'+esc(e.title||'제목 없는 기록')+'</h3><p>'+esc(e.body.slice(0,350))+'</p><button class="ghost" data-action="entry" data-id="'+esc(e.id)+'">열기</button></div>').join(''):'<div class="empty"><div class="empty-art">✧</div><h2>지난 기록이 없습니다</h2><p>오늘을 기록해 두면 내년에 다시 만날 수 있습니다.</p></div>')+'</div>'; }
function trash(c) { const items=data.entries.filter(e=>e.deleted_at); c.innerHTML='<div class="wide-view"><div class="wide-head"><h2>휴지통</h2><span class="subtle">'+items.length+'개</span><span class="spacer"></span>'+(items.length?'<button class="ghost danger" data-action="empty-trash">휴지통 비우기</button>':'')+'</div>'+(items.length?items.map(e=>'<div class="panel-card"><span class="subtle">'+esc(dateLabel(e.date))+'</span><h3>'+esc(e.title||'제목 없는 기록')+'</h3><button class="ghost" data-action="restore" data-id="'+esc(e.id)+'">복원</button></div>').join(''):'<div class="empty"><p>휴지통이 비어 있습니다.</p></div>')+'</div>'; }
function settings(c) {
  const s = data.sync_status || {};
  const nav = [
    ['profile', '☺ 별명 설정', data.nickname?.trim() ? '현재 별명: ' + data.nickname.trim() : '별명 미설정 (안녕하세요!)'],
    ['sync', '☁ Google Drive 동기화', (s.connected ? accountLabel(s) : 'Google 로그인 가능') + (data.sync_conflicts?.length ? ' · 충돌 ' + data.sync_conflicts.length + '건' : '')],
    ['backup', '⇅ 가져오기와 백업', 'Day One 및 Niki 백업 파일 관리'],
    ['templates', '▤ 템플릿', '저장된 서식 ' + data.templates.length + '개'],
    ['lock', '🔒 앱 잠금', data.lock_enabled ? '화면 잠금 사용 중' : '화면 잠금 해제됨'],
    ['about', 'ⓘ 앱 정보', 'v' + (data.app_version || '0.1.0') + (updateInfo?.update_available ? ' · ✨ v' + updateInfo.latest_version + ' 업데이트 가능' : ' · 자동 업데이트 관리')]
  ];
  c.innerHTML='<section class="list-pane"><div class="list-head"><h2>설정 목록</h2><div class="subtle">항목을 선택하세요</div></div><div class="entry-list">'
    +nav.map(([key,title,desc])=>'<button class="entry-card '+(settingsTab===key?'active':'')+'" data-action="settings-tab" data-id="'+key+'"><h3>'+esc(title)+'</h3><p>'+esc(desc)+'</p></button>').join('')
    +'</div></section><section class="wide-view" id="settings-detail"></section>';
  const detail=document.getElementById('settings-detail');
  if(settingsTab==='profile') detail.innerHTML='<div class="wide-head"><h2>별명 설정</h2></div><div class="panel-card"><h3>좌측 메뉴 상단 별명</h3><p>좌측 메뉴바 상단에 표시할 이름이나 별명을 설정합니다. 비워 두면 기본 문구인 <strong>안녕하세요!</strong>가 표시됩니다.</p><input id="profile-nickname" maxlength="40" placeholder="안녕하세요!" value="'+esc(data.nickname||'')+'"><div class="panel-actions"><button class="primary" data-action="save-nickname">별명 저장</button>'+(data.nickname?.trim()?'<button class="ghost" data-action="clear-nickname">초기화 (안녕하세요!)</button>':'')+'</div></div>';
  else if(settingsTab==='sync') renderSyncSettings(detail);
  else if(settingsTab==='backup') detail.innerHTML='<div class="wide-head"><h2>가져오기와 백업</h2></div><div class="panel-card"><h3>파일 가져오기 및 백업</h3><p>Day One JSON/ZIP 파일 또는 Niki 백업을 가져옵니다. 백업에는 첨부 파일도 들어갑니다.</p><div class="panel-actions"><button class="ghost" data-action="import">파일 가져오기</button><button class="primary" data-action="backup">백업 만들기</button></div></div>';
  else if(settingsTab==='templates') detail.innerHTML='<div class="wide-head"><h2>템플릿</h2></div><div class="panel-card"><h3>사용자 템플릿 관리</h3><p>반복해서 쓰는 질문과 서식을 저장합니다.</p>'+data.templates.map(t=>'<div style="display:flex;gap:8px;align-items:center;margin:6px 0"><span style="flex:1">'+esc(t.name)+'</span><button class="ghost" data-action="edit-template" data-id="'+esc(t.id)+'">수정</button><button class="ghost danger" data-action="delete-template" data-id="'+esc(t.id)+'">삭제</button></div>').join('')+'<button class="ghost" data-action="new-template">＋ 템플릿 만들기</button></div>';
  else if(settingsTab==='lock') detail.innerHTML='<div class="wide-head"><h2>앱 잠금</h2></div><div class="panel-card"><h3>화면 잠금 암호</h3><p>화면 접근을 막는 암호입니다. 로컬 저장 파일 자체는 암호화하지 않습니다.</p><input type="password" id="current-password" placeholder="현재 암호"><input type="password" id="new-password" placeholder="새 암호 (8자 이상, 비우면 해제)"><div class="panel-actions"><button class="ghost" data-action="set-password">암호 저장</button>'+(data.lock_enabled?'<button class="ghost" data-action="lock">지금 잠그기</button>':'')+'</div></div>';
  else renderAboutSettings(detail);
}
function renderAboutSettings(pane) {
  const cfg = data.updater_config || {};
  const ver = data.app_version || '0.1.0';
  const isMac = /Mac/i.test(navigator.platform || navigator.userAgent || '');
  const hostName = isMac ? 'Mac' : 'PC';
  const installLabel = data.installed_on_pc
    ? (isMac ? 'Applications 설치본 갱신' : 'PC 설치본 갱신 및 바로가기 재생성')
    : (isMac ? '이 Mac에 설치하기 (/Applications 등록)' : '이 PC에 설치하기 (바탕화면·시작 메뉴 등록)');
  pane.innerHTML = '<div class="wide-head"><h2>앱 정보</h2></div>'
    +'<div class="panel-card"><h3>Niki v'+esc(ver)+'</h3>'
    +'<p>구독과 광고 없이 로컬에서 사용하며, 선택 시 사용자의 Google Drive(<code>appDataFolder</code>)를 통해 256비트 종단간 암호화 동기화를 지원합니다.</p>'
    +'<p><strong>'+hostName+' 설치 상태:</strong> '+(data.running_installed ? '<span class="badge-connected">● '+hostName+' 설치 버전 실행 중</span>' : data.installed_on_pc ? '설치됨 (현재 빌드 폴더에서 실행 중)' : '미설치')+(data.install_path?'<br><span class="subtle">설치 경로: '+esc(data.install_path)+'</span>':'')+'</p>'
    +(updateInfo ? '<p><strong>최신 확인 버전:</strong> v'+esc(updateInfo.latest_version)+' '+(updateInfo.update_available ? '<span class="badge-connected">업데이트 가능</span>' : '(최신 버전 사용 중)')+(updateInfo.notes?'<br><span class="subtle">'+esc(updateInfo.notes)+'</span>':'')+'</p>' : '')
    +'<div class="panel-actions">'
    +'<button class="ghost" data-action="updater-check-now">지금 업데이트 확인</button>'
    +(updateInfo?.update_available ? '<button class="primary" data-action="updater-install">✨ v'+esc(updateInfo.latest_version)+' 다운로드 및 재시작</button>' : '')
    +'<button class="ghost" data-action="install-to-pc">'+installLabel+'</button>'
    +'</div></div>';
  /* [개발자 전용 UI: 업데이트 서버 설정 및 latest.json 매니페스트 생성 도구]
  pane.innerHTML += '<div class="panel-card"><h3>업데이트 서버 설정 (GitHub Releases / 자체 웹 서버)</h3>'
    +'<p>처음에는 GitHub 저장소를 입력해 사용하고, 추후 별도 웹 서버로 이전할 때 해당 서버의 <code>latest.json</code> 주소로 바꾸면 됩니다. 프로젝트 루트의 <code>updater-config.json</code>에 적어 빌드하면 모든 사용자에게 기본 적용됩니다.</p>'
    +'<input id="updater-endpoint" placeholder="예: myid/niki 또는 https://updates.example.com/niki/latest.json" value="'+esc(cfg.endpoint||'')+'">'
    +'<input id="updater-pubkey" placeholder="Ed25519 서명 검증 공개키 (Base64, 선택 사항)" value="'+esc(cfg.pubkey||'')+'">'
    +'<label class="check-row"><input type="checkbox" id="updater-auto-check" '+(cfg.auto_check!==false?'checked':'')+'><span>앱을 켤 때 자동으로 새 버전 확인 및 알림 표시</span></label>'
    +'<div class="panel-actions"><button class="primary" data-action="updater-save-config">업데이트 설정 저장</button></div>'
    +'<details class="oauth-details"><summary>개발자 배포 도구 (서명 키 및 latest.json 매니페스트 생성)</summary>'
    +'<div class="oauth-details-body">'
    +'<p style="margin:0 0 8px">새 버전을 빌드한 뒤 GitHub Releases나 자체 웹 서버에 올릴 <code>Niki_Setup.exe</code> 설치 파일과 <code>latest.json</code> 파일(SHA-256 해시 및 Ed25519 디지털 서명 포함)을 생성합니다.</p>'
    +'<div class="panel-actions" style="margin-bottom:8px"><button class="ghost" data-action="updater-gen-keypair">새 Ed25519 서명 키 쌍 생성</button></div>'
    +(generatedKeyPair ? '<p><strong>공개키 (updater-config.json의 pubkey에 저장):</strong><br><code style="user-select:all;word-break:break-all">'+esc(generatedKeyPair.public_key_base64)+'</code><br><strong>개인키 (개발자만 안전하게 보관):</strong><br><code style="user-select:all;word-break:break-all">'+esc(generatedKeyPair.private_key_pkcs8_base64)+'</code></p>' : '')
    +'<input id="manifest-version" placeholder="배포할 새 버전 (예: 0.1.1)" value="'+esc(ver)+'">'
    +'<input id="manifest-url" placeholder="다운로드 URL (예: https://github.com/Nonchrono/niki/releases/download/v0.1.1/Niki_Setup.exe)">'
    +'<input id="manifest-notes" placeholder="업데이트 안내 문구 (예: 새로운 기능 추가 및 버그 수정)">'
    +'<input id="manifest-privkey" type="password" placeholder="Ed25519 개인키 (Base64, 비워 두면 SHA-256 해시만 포함)" value="'+esc(generatedKeyPair?.private_key_pkcs8_base64||'')+'">'
    +'<div class="panel-actions"><button class="ghost" data-action="updater-build-manifest">Niki_Setup.exe 및 latest.json 생성</button></div>'
    +(generatedManifest ? '<textarea readonly style="margin-top:10px;font-family:monospace;font-size:11px;min-height:140px">'+esc(generatedManifest)+'</textarea>' : '')
    +'</div></details></div>';
  */
}
function entryInput() { const e=selected(); if(!e) return null; return {id:e.id,journalId:document.getElementById('entry-journal')?.value||e.journal_id,title:document.getElementById('entry-title')?.value??e.title,body:document.getElementById('entry-body')?.value??e.body,date:document.getElementById('entry-date')?.value||e.date,favorite:e.favorite,tags:(document.getElementById('entry-tags')?.value||e.tags.join(',')).split(',').map(x=>x.trim()).filter(Boolean),location:document.getElementById('entry-location')?.value??e.location,weather:document.getElementById('entry-weather')?.value??e.weather}; }
function scheduleSave() { const status=document.getElementById('save-status'); if(status) status.textContent='저장 대기 중…'; clearTimeout(saveTimer); saveTimer=setTimeout(saveCurrent,550); }
async function saveCurrent() { clearTimeout(saveTimer); saveTimer=null; if(saving){saveAgain=true;return;} const input=entryInput(); if(!input)return; saving=true; const status=document.getElementById('save-status'); if(status)status.textContent='저장 중…'; try{await invoke('save_entry',{input}); await reload(); if(entryInput()&&JSON.stringify(entryInput())!==JSON.stringify(input))saveAgain=true; if(status)status.textContent='모든 변경 사항 저장됨'+entrySyncBadge(input.id);}catch(e){fail(e);if(status)status.textContent='저장 실패';}finally{saving=false;if(saveAgain){saveAgain=false;setTimeout(saveCurrent,0);}} }
async function leave() {
  if (!saveTimer && !saving && !saveAgain) return;
  clearTimeout(saveTimer);
  saveTimer = null;
  while (saving) await new Promise(r=>setTimeout(r,25));
  await saveCurrent();
  while (saving) await new Promise(r=>setTimeout(r,25));
}
async function newEntry() { await leave(); const journalId=journal==='all'?data.journals[0]?.id:journal; if(!journalId)return toast('일지가 없습니다',true); entryId=await invoke('save_entry',{input:{journalId,title:'',body:'',date:localDate(),favorite:false,tags:[],location:'',weather:''}}); await reload(); view='timeline'; dateFilter=''; preview=false; render(); document.getElementById('entry-title')?.focus(); }
async function navigate(next,id) { await leave(); view=next; if(id)journal=id; dateFilter=''; render(); }
function format(kind) { const el=document.getElementById('entry-body'); if(!el)return; const a=el.selectionStart,b=el.selectionEnd,t=el.value.slice(a,b),v={bold:'**'+(t||'굵은 글씨')+'**',italic:'*'+(t||'기울임')+'*',heading:'## '+(t||'제목'),quote:'> '+(t||'인용'),list:'- '+(t||'항목'),check:'- [ ] '+(t||'할 일')}[kind]; el.setRangeText(v,a,b,'select'); el.focus(); scheduleSave(); }
function toBase64(file) { return new Promise((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result).split(',')[1]);reader.onerror=()=>reject(reader.error);reader.readAsDataURL(file);}); }
document.addEventListener('contextmenu',ev=>{
  const row=ev.target.closest('[data-action="journal"]');
  closeContextMenu();
  if(!row)return;
  ev.preventDefault();
  const id=row.dataset.id, j=data.journals.find(x=>x.id===id);
  if(!j)return;
  const menu=document.createElement('div');
  menu.className='context-menu';
  menu.innerHTML='<div class="context-menu-head"><span class="dot" style="background:'+esc(j.color)+'"></span><span>'+esc(j.name)+'</span></div>'
    +'<button class="context-menu-item" data-action="edit-journal" data-id="'+esc(j.id)+'">✎ 수정</button>'
    +'<button class="context-menu-item danger" data-action="delete-journal" data-id="'+esc(j.id)+'">× 삭제</button>';
  document.body.appendChild(menu);
  const rect=menu.getBoundingClientRect();
  menu.style.left=Math.max(8,Math.min(ev.clientX,window.innerWidth-rect.width-8))+'px';
  menu.style.top=Math.max(8,Math.min(ev.clientY,window.innerHeight-rect.height-8))+'px';
});
document.addEventListener('click',async ev=>{const b=ev.target.closest('[data-action]');closeContextMenu();if(!b)return;if(b.tagName==='A')ev.preventDefault();const a=b.dataset.action,id=b.dataset.id;try{
  if(a==='unlock'){await invoke('unlock',{password:document.getElementById('unlock-password').value});await reload();render();autoCheckUpdate();autoSyncDrive();}
  else if(a==='toggle-sidebar'){sidebarCollapsed=!sidebarCollapsed;render();}
  else if(a==='updater-save-config'){const ep=document.getElementById('updater-endpoint')?.value||'',pk=document.getElementById('updater-pubkey')?.value||'',ac=document.getElementById('updater-auto-check')?.checked??true;await invoke('updater_save_config',{endpoint:ep,pubkey:pk,autoCheck:ac});await reload();render();toast('업데이트 서버 설정이 저장되었습니다');}
  else if(a==='updater-check-now'){b.disabled=true;try{toast('업데이트 확인 중…');updateInfo=await invoke('updater_check');updateDismissed=false;render();toast(updateInfo.update_available?'새 버전(v'+updateInfo.latest_version+') 업데이트가 있습니다!':'현재 최신 버전(v'+updateInfo.current_version+')을 사용 중입니다');}finally{b.disabled=false;}}
  else if(a==='updater-install'){await leave();b.disabled=true;try{toast('새 버전 다운로드 및 검증 중… 완료 후 자동으로 재시작됩니다.');const ver=await invoke('updater_install');toast('v'+ver+' 업데이트 완료! 앱을 재시작합니다…');}finally{b.disabled=false;}}
  else if(a==='updater-dismiss'){updateDismissed=true;render();}
  else if(a==='updater-skip'&&updateInfo?.latest_version){await invoke('updater_skip_version',{version:updateInfo.latest_version});await reload();render();toast('v'+updateInfo.latest_version+' 버전 알림을 건너뜁니다');}
  else if(a==='updater-gen-keypair'){generatedKeyPair=await invoke('updater_generate_keypair');await reload();render();toast('Ed25519 서명 키 쌍이 생성되고 공개키가 등록되었습니다');}
  else if(a==='updater-build-manifest'){const ver=document.getElementById('manifest-version')?.value||'',url=document.getElementById('manifest-url')?.value||'',notes=document.getElementById('manifest-notes')?.value||'',pk=document.getElementById('manifest-privkey')?.value||'';generatedManifest=await invoke('updater_create_manifest',{version:ver,downloadUrl:url,notes,privateKey:pk});render();toast('Niki_Setup.exe 및 latest.json이 실행 파일 폴더에 생성되었습니다');}
  else if(a==='install-to-pc'){await leave();b.disabled=true;try{toast('PC에 Niki를 설치하고 바로가기를 생성하는 중…');const p=await invoke('install_app_to_pc');await reload();render();toast('설치 완료: '+p);}finally{b.disabled=false;}}
  else if(a==='google-login'){const cid=document.getElementById('drive-client-id')?.value?.trim()||'',sec=document.getElementById('drive-client-secret')?.value||'';if(!cid&&!data.sync_status?.has_oauth_client){const det=document.querySelector('.oauth-details');if(det)det.open=true;document.getElementById('drive-client-id')?.focus();toast('Google Cloud에서 발급받은 OAuth 클라이언트 ID를 먼저 입력해 주세요',true);return;}await startGoogleLogin(cid,sec,b);}
  else if(a==='drive-disconnect'){await invoke('drive_disconnect');visibleSyncKey='';await reload();toast('Google 계정에서 로그아웃되었습니다');render();}
  else if(a==='drive-sync'){await leave();b.disabled=true;try{toast('Google Drive와 암호화 동기화 중…');const res=await invoke('drive_sync_now');await reload();render();const msg='동기화 완료 (업로드 '+res.uploaded_revisions+'건, 다운로드 '+res.downloaded_revisions+'건'+(res.conflicts_created?', 충돌 '+res.conflicts_created+'건':'')+')';toast(res.warnings?.length?msg+' · '+res.warnings[0]:msg, Boolean(res.warnings?.length));}catch(e){await reload();render();throw e;}finally{b.disabled=false;}}
  else if(a==='drive-toggle-pause'){await invoke('drive_set_paused',{paused:!data.sync_status?.sync_paused});await reload();render();toast(data.sync_status?.sync_paused?'동기화가 일시 중지되었습니다':'동기화가 재개되었습니다');}
  else if(a==='drive-show-key'){if(visibleSyncKey){visibleSyncKey='';render();}else{const k=await invoke('drive_get_sync_key');if(!k)toast('설정된 복구 키가 없습니다. 먼저 생성하거나 입력하세요',true);else{visibleSyncKey=k;render();}}}
  else if(a==='drive-generate-key'){if(!data.sync_status?.has_sync_key||confirm('새 복구 키를 생성하면 기존 원격 암호화 파일과 키가 달라집니다. 계속할까요?')){visibleSyncKey=await invoke('drive_generate_sync_key');await reload();render();toast('새 복구 키가 생성되었습니다. 안전한 곳에 보관하세요');}}
  else if(a==='drive-set-key'){const val=document.getElementById('drive-recovery-key')?.value||'';await invoke('drive_set_sync_key',{recoveryKey:val});visibleSyncKey='';await reload();render();toast('복구 키가 등록되었습니다');}
  else if(a==='resolve-conflict'){await leave();await invoke('resolve_sync_conflict',{conflictId:id,choice:b.dataset.choice});await reload();render();toast('충돌을 해결했습니다');}
  else if(a==='lock'){await leave();await invoke('lock');await reload();render();}
  else if(a==='all')await navigate('timeline','all'); else if(a==='journal')await navigate('timeline',id);
  else if(['calendar','media','memories','trash','settings'].includes(a))await navigate(a);
  else if(a==='settings-tab'){settingsTab=id;settings(document.getElementById('content'));}
  else if(a==='new-entry')await newEntry(); else if(a==='entry'){await leave();entryId=id;view='timeline';dateFilter='';preview=false;render();}
  else if(a==='favorites'){favorites=!favorites;timeline();} else if(a==='clear-date'){dateFilter='';timeline();}
  else if(a==='calendar-day'){dateFilter=b.dataset.date;view='timeline';render();}
  else if(a==='prev-month'||a==='next-month'||a==='today-month'){month=a==='today-month'?new Date(new Date().getFullYear(),new Date().getMonth(),1):new Date(month.getFullYear(),month.getMonth()+(a==='next-month'?1:-1),1);calendar(document.getElementById('content'));}
  else if(a==='favorite-entry'){const e=selected();e.favorite=!e.favorite;b.textContent=e.favorite?'♥':'♡';scheduleSave();}
  else if(a==='preview'){await leave();preview=!preview;editor();} else if(a==='format')format(b.dataset.format);
  else if(a==='attach')attachFile.click(); else if(a==='print')window.print();
  else if(a==='trash-entry'&&confirm('이 기록을 휴지통으로 이동할까요?')){await invoke('trash_entry',{id:entryId});entryId=null;await reload();render();}
  else if(a==='restore'){await invoke('restore_entry',{id});await reload();render();toast('기록을 복원했습니다');}
  else if(a==='empty-trash'&&confirm('휴지통의 모든 기록을 영구 삭제할까요? 삭제 후에는 복원할 수 없습니다.')){const count=await invoke('empty_trash');media.clear();await reload();render();toast('휴지통에서 '+count+'개의 기록을 영구 삭제했습니다');}
  else if(a==='remove-attachment'&&confirm('첨부 파일을 삭제할까요?')){await invoke('remove_attachment',{entryId,attachmentId:id});media.delete(id);await reload();editor();}
  else if(a==='new-journal'){const name=prompt('새 일지 이름');if(name?.trim()){const id=await invoke('save_journal',{id:null,name,color:'#576b61'});await reload();await navigate('timeline',id);}}
  else if(a==='edit-journal'){const j=data.journals.find(x=>x.id===id),name=prompt('일지 이름',j.name);if(name?.trim()){const color=prompt('색상 코드 (#RRGGBB)',j.color);if(color){await invoke('save_journal',{id,name,color});await reload();render();}}}
  else if(a==='delete-journal'&&confirm('빈 일지를 삭제할까요?')){await invoke('delete_journal',{id});if(journal===id)journal='all';await reload();render();}
  else if(a==='import')importFile.click(); else if(a==='backup'&&confirm('백업 파일은 암호화되지 않은 JSON이며 사진과 일기 내용이 그대로 포함됩니다. 안전한 저장 위치에서만 사용하세요. 백업을 만들까요?')){const path=await invoke('export_backup');toast('백업 저장: '+path);}
  else if(a==='new-template'){const name=prompt('템플릿 이름');if(name?.trim()){const body=prompt('템플릿 내용 (Markdown 가능)')??'';await invoke('save_template',{id:null,name,body});await reload();render();}}
  else if(a==='edit-template'){const t=data.templates.find(x=>x.id===id),name=prompt('템플릿 이름',t.name);if(name?.trim()){const body=prompt('템플릿 내용',t.body);if(body!==null){await invoke('save_template',{id,name,body});await reload();render();}}}
  else if(a==='delete-template'&&confirm('템플릿을 삭제할까요?')){await invoke('delete_template',{id});await reload();render();}
  else if(a==='save-nickname'){const val=document.getElementById('profile-nickname')?.value||'';await invoke('set_nickname',{nickname:val});await reload();render();toast(data.nickname?'별명이 저장되었습니다':'기본 인사말(안녕하세요!)로 설정되었습니다');}
  else if(a==='clear-nickname'){await invoke('set_nickname',{nickname:''});await reload();render();toast('기본 인사말(안녕하세요!)로 초기화되었습니다');}
  else if(a==='set-password'){await invoke('set_lock_password',{currentPassword:document.getElementById('current-password').value,newPassword:document.getElementById('new-password').value});await reload();render();toast('잠금 설정 저장됨');}
}catch(e){fail(e);}});
document.addEventListener('input',ev=>{if(ev.target.id==='search'){const pos=ev.target.selectionStart;search=ev.target.value;timeline();const field=document.getElementById('search');field.focus();field.setSelectionRange(pos,pos);}else if(['entry-title','entry-body','entry-date','entry-tags','entry-location','entry-weather'].includes(ev.target.id))scheduleSave();});
document.addEventListener('change',ev=>{if(ev.target.id==='tag-filter'){tag=ev.target.value;timeline();}else if(ev.target.id==='entry-journal')scheduleSave();else if(ev.target.id==='insert-template'&&ev.target.value){const t=data.templates.find(x=>x.id===ev.target.value),body=document.getElementById('entry-body');if(t&&body){body.value=t.body+(body.value?'\n\n'+body.value:'');scheduleSave();}ev.target.value='';}});
document.addEventListener('keydown',ev=>{const mod=ev.ctrlKey||ev.metaKey;if(ev.key==='Escape')closeContextMenu();if(ev.key==='Enter'&&ev.target.id==='unlock-password')document.querySelector('[data-action="unlock"]')?.click();if(ev.key==='Enter'&&ev.target.id==='profile-nickname')document.querySelector('[data-action="save-nickname"]')?.click();if(mod&&ev.key.toLowerCase()==='n'){ev.preventDefault();newEntry().catch(fail);}if(mod&&ev.key.toLowerCase()==='s'){ev.preventDefault();saveCurrent().then(()=>toast('저장했습니다'));}});
importFile.addEventListener('change',async()=>{const f=importFile.files?.[0];if(!f)return;try{toast('가져오는 중…');const result=await invoke('import_data',{name:f.name,data:await toBase64(f)});await reload();render();toast(result.entries_added+'개 글, '+result.media_added+'개 첨부 가져옴'+(result.warnings.length?' · 경고 '+result.warnings.length+'건':''));if(result.warnings.length)console.warn(result.warnings);}catch(e){fail(e);}finally{importFile.value='';}});
attachFile.addEventListener('change',async()=>{const id=entryId;if(!id)return;try{await leave();for(const f of attachFile.files||[]){await invoke('add_attachment',{entryId:id,name:f.name,mime:f.type||'application/octet-stream',data:await toBase64(f)});}await reload();editor();toast('첨부 파일 저장됨');}catch(e){fail(e);}finally{attachFile.value='';}});
document.addEventListener('click',async ev=>{
  const tile=ev.target.closest('.attach-tile');
  if(!tile||ev.target.closest('button'))return;
  const id=tile.id.replace('att-',''), attachment=selected()?.attachments.find(a=>a.id===id);
  if(!attachment)return;
  try{
    const url=await getMedia(id), popup=document.createElement('div');
    popup.className='media-overlay';
    popup.innerHTML='<div class="media-dialog"><div class="media-dialog-head"><strong></strong><button class="ghost" data-close="1">닫기</button></div><div class="media-content"></div><a class="ghost">파일 저장</a></div>';
    popup.querySelector('strong').textContent=attachment.name;
    const content=popup.querySelector('.media-content');
    let element;
    if(attachment.mime.startsWith('image/')){element=document.createElement('img');element.alt=attachment.name;}
    else if(attachment.mime.startsWith('video/')){element=document.createElement('video');element.controls=true;element.autoplay=true;}
    else if(attachment.mime.startsWith('audio/')){element=document.createElement('audio');element.controls=true;element.autoplay=true;}
    else if(attachment.mime==='application/pdf'){element=document.createElement('iframe');element.title=attachment.name;}
    else {element=document.createElement('p');element.textContent='미리보기를 지원하지 않는 파일입니다.';}
    if(element.localName!=='p')element.src=url;
    content.appendChild(element);
    const download=popup.querySelector('a');download.href=url;download.download=attachment.name;
    popup.addEventListener('click',event=>{if(event.target===popup||event.target.closest('[data-close]'))popup.remove();});
    document.body.appendChild(popup);
  }catch(e){fail(e);}
});
if(window.__TAURI__?.core?.invoke){reload().then(()=>{render();autoCheckUpdate();autoSyncDrive();}).catch(fail);}else app.innerHTML='<div class="empty"><h2>데스크톱 앱에서 열어 주세요</h2></div>';
