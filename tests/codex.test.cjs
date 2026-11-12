const { chromium } = require('playwright-core');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');
const fs = require('node:fs');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', args: ['--allow-file-access-from-files'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.route('https://**/*', route => route.abort());
    await page.addInitScript(() => {
      window.calls = []; window.failSave = false; window.deferSummary = false; window.failChat = false;
      window.chatHistory = []; window.memoryLeft = 0; window.memoryModel = { installed: true, downloading: false, failed: false, downloaded_bytes: 0, total_bytes: 111821878 };
      window.codexStatus = { installed: true, connected: false, plan: null, login_pending: false };
      const entry = (id, title, body) => ({ id, journal_id: 'j1', title, body, date: '2026-11-10', created_at: 'v1', updated_at: 'v1', tags: [], favorite: false, location: '', weather: '', attachments: [], deleted_at: null });
      window.sample = {
        locked: false, nickname: '나', app_version: '0.1.22', updater_config: {},
        journals: [{ id: 'j1', name: '매일 일기', color: '#576b61' }, { id: 'j2', name: '여행 일기', color: '#576b61' }],
        entries: [entry('e1', '산책', '공원에서 산책했다.'), { ...entry('e2', '다른 일기', '다른 비공개 내용'), journal_id: 'j2' }],
        templates: [], sync_conflicts: [], pending_resolution_count: 0, synced_entry_ids: [],
        sync_status: { connected: false, sync_paused: false, pending_upload_count: 0 },
      };
      window.__TAURI__ = { core: { invoke: async (name, args) => {
        calls.push({ name, args });
        if (name === 'bootstrap') return structuredClone(sample);
        if (name === 'codex_status') return structuredClone(codexStatus);
        if (name === 'codex_conversation') return structuredClone(chatHistory);
        if (name === 'codex_connect') { codexStatus.login_pending = true; return { verification_url: 'https://auth.openai.com/codex/device', user_code: 'DEMO-CODE' }; }
        if (name === 'codex_disconnect') { Object.assign(codexStatus, { connected: false, plan: null, login_pending: false }); return; }
        if (name === 'save_entry_checked') {
          if (failSave) throw Error('다른 기기에서 이 일기가 변경되었습니다');
          const current = sample.entries.find(e => e.id === args.input.id);
          Object.assign(current, { title: args.input.title, body: args.input.body, updated_at: 'v2' });
          return { id: current.id, entry: structuredClone(current) };
        }
        if (name === 'codex_summarize') {
          chatHistory.push({ id: `m${chatHistory.length}`, role: 'user', kind: 'summary', content: `「${sample.entries.find(e => e.id === args.entryId).title}」 요약해 줘` });
          const answer = deferSummary ? await new Promise(resolve => { window.completeSummary = resolve; }) : '<script>window.injected = true</script> 산책하며 편안한 하루를 보냈다.';
          chatHistory.push({ id: `m${chatHistory.length}`, role: 'assistant', kind: 'summary', content: answer });
          return structuredClone(chatHistory);
        }
        if (name === 'codex_prefs') return { consented: true, excluded_journals: [], model: null, default_model: 'gpt-6-luna', model_fallback: false, background: true };
        if (name === 'codex_cli_info') return { path: '/usr/local/bin/codex', version: '0.167.1', supported: true, custom: false, min_version: '0.167' };
        if (name === 'codex_memory_info') return { model_installed: true, model_bytes: 1, index_bytes: 1, indexed_entries: 0, total_entries: 0, downloading: false, failed: false, months_summarized: 0, chat_summarized: false };
        if (name === 'codex_memory_model') return structuredClone(memoryModel);
        if (name === 'codex_refresh_memory') {
          if (window.memoryLeft > 0) { window.memoryLeft -= 1; await new Promise(r => setTimeout(r, 120)); }
          return { remaining: window.memoryLeft || 0 };
        }
        if (name === 'codex_chat') {
          window.memoryLeftAtChat = window.memoryLeft;
          window.refreshCallsAtChat = calls.filter(c => c.name === 'codex_refresh_memory').length;
          chatHistory.push({ id: `m${chatHistory.length}`, role: 'user', kind: 'chat', content: args.message });
          if (failChat) throw Error('연결을 확인하고 다시 시도해 주세요.');
          chatHistory.push({ id: `m${chatHistory.length}`, role: 'assistant', kind: 'chat', content: '산책 이야기를 이어가 볼까요?' });
          return structuredClone(chatHistory);
        }
        throw Error(`Unexpected command: ${name}`);
      } } };
    });
    await page.goto(pathToFileURL(path.resolve('ui/index.html')).href + '#/all/entries/e1');
    await page.locator('#entry-title').waitFor();
    await page.waitForFunction(() => calls.some(c => c.name === 'codex_status'));
    assert.equal(await page.locator('[data-action="open-summary"]').count(), 0, 'disconnected diary must not offer summaries');
    await page.evaluate(() => { codexStatus.login_pending = true; window.dispatchEvent(new Event('focus')); });
    await page.waitForFunction(() => calls.filter(c => c.name === 'codex_status').length >= 2);
    await page.evaluate(() => Object.assign(codexStatus, { connected: true, plan: 'free', login_pending: false }));
    await page.locator('[data-action="open-summary"]').waitFor();
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('#diary-summary-sidebar').waitFor();
    await page.evaluate(() => { codexStatus.connected = false; window.dispatchEvent(new Event('focus')); });
    await page.waitForFunction(() => !document.querySelector('#diary-summary-sidebar') && !document.querySelector('[data-action="open-summary"]'));
    assert.equal(await page.locator('#diary-summary-sidebar').count(), 0, 'disconnect must remove sidebar and its contents');
    await page.evaluate(() => location.hash = '#/settings/codex');
    await page.locator('[data-action="codex-connect"]').click();
    assert.equal(await page.getByRole('heading', { name: 'ChatGPT 계정', exact: true }).count(), 1);
    await page.locator('[data-testid="data-details"] > summary').click();
    assert.ok((await page.locator('[data-testid="data-details"]').innerText()).includes('OpenAI'));
    assert.ok(!(await page.locator('#settings-detail').innerText()).includes('Niki의 로그인은'));
    await page.locator('.codex-device-code').waitFor();
    assert.equal(await page.locator('.codex-device-code').innerText(), 'DEMO-CODE');
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_open_login').length), 0, 'login must not open a browser automatically');
    await page.evaluate(() => Object.assign(codexStatus, { connected: true, plan: 'free', login_pending: false }));
    await page.getByText('연결됨 · free').waitFor();
    assert.equal(await page.locator('.codex-device-code').count(), 0);
    await page.evaluate(() => location.hash = '#/all/entries/e1');
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('[data-action="codex-summarize"]').waitFor();
    assert.equal(await page.locator('#diary-summary-sidebar textarea').count(), 1);
    assert.equal(await page.locator('#diary-summary-sidebar h3').innerText(), 'Niki와 대화');
    assert.equal(await page.locator('[data-action="open-summary"]').getAttribute('aria-expanded'), 'true');
    assert.equal(await page.locator('.topbar [data-action="open-summary"]').count(), 1, 'chat is available in the top bar');
    assert.equal(await page.locator('.codex-summary-head [data-action="codex-summarize"]').count(), 1);
    assert.ok(!(await page.locator('#diary-summary-sidebar').innerText()).includes('GPT-6 Luna'));
    assert.equal(await page.getByRole('button', { name: '요약하기', exact: true }).count(), 0);
    assert.ok(!(await page.locator('#diary-summary-sidebar').innerText()).includes('OpenAI에 보냅니다'));
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_summarize').length), 0, 'opening panel must not send diary content');
    await page.locator('#entry-title').fill('산책과 독서');
    await page.locator('[data-action="codex-summarize"]').click();
    await page.locator('[data-testid="codex-summary-result"]').waitFor();
    const result = await page.evaluate(() => ({ calls, entries: sample.entries, injected: window.injected, storage: { ...localStorage } }));
    const request = result.calls.find(c => c.name === 'codex_summarize');
    assert.deepEqual(request.args, { entryId: 'e1', expectedUpdatedAt: 'v2' });
    assert.ok(result.calls.findIndex(c => c.name === 'save_entry_checked') < result.calls.findIndex(c => c.name === 'codex_summarize'));
    assert.equal(result.entries[0].body, '공원에서 산책했다.', 'summary must not overwrite diary');
    assert.equal(result.entries[1].body, '다른 비공개 내용');
    assert.equal(result.injected, undefined, 'model output is text, never executable HTML');
    assert.ok(!JSON.stringify(result.storage).includes('script'), 'summary must not enter localStorage');
    fs.mkdirSync('output/codex-ui', { recursive: true });
    await page.screenshot({ path: 'output/codex-ui/summary.png' });
    const checkSidebar = async () => {
      const layout = await page.evaluate(() => {
        const editor = document.querySelector('.editor-document').getBoundingClientRect();
        const panelEl = document.querySelector('#diary-summary-sidebar');
        const panel = panelEl.getBoundingClientRect();
        const hit = document.elementFromPoint(panel.left + panel.width / 2, panel.top + 120);
        return { editorRight: editor.right, panelLeft: panel.left, panelRight: panel.right, panelWidth: panel.width, covered: panelEl.contains(hit), top: panel.top, topbarBottom: document.querySelector('.topbar').getBoundingClientRect().bottom, width: innerWidth, overflow: document.documentElement.scrollWidth - innerWidth };
      });
      assert.ok(layout.covered, 'panel overlays the diary (higher z-index): ' + JSON.stringify(layout));
      assert.ok(layout.editorRight > layout.panelLeft, 'panel overlaps instead of resizing the diary: ' + JSON.stringify(layout));
      assert.ok(Math.abs(layout.panelWidth - Math.min(435, layout.width - 250)) < 2 || layout.panelWidth <= 435, JSON.stringify(layout));
      assert.ok(layout.panelRight <= layout.width && layout.overflow <= 0 && layout.top >= layout.topbarBottom - 1, JSON.stringify(layout));
    };
    await checkSidebar();
    await page.setViewportSize({ width: 900, height: 650 });
    await checkSidebar();
    await page.screenshot({ path: 'output/codex-ui/summary-narrow.png' });
    await page.setViewportSize({ width: 1280, height: 800 });
    const editorWidth = () => page.evaluate(() => document.querySelector('.editor-document').getBoundingClientRect().width);
    const widthOpen = await editorWidth();
    assert.ok(await page.locator('.list-pane').isVisible(), 'diary list stays in place under the overlay');
    const timing = await page.evaluate(() => getComputedStyle(document.querySelector('#diary-summary-sidebar')).transitionTimingFunction);
    assert.ok(/ease-in-out|cubic-bezier\(0\.42, 0, 0\.58, 1\)/.test(timing), timing);
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();
    await page.waitForFunction(() => getComputedStyle(document.querySelector('#diary-summary-sidebar')).visibility === 'hidden');
    assert.equal(await editorWidth(), widthOpen, 'opening the panel must not change diary layout');
    assert.equal(await page.locator('[data-action="open-summary"] svg path').count(), 3, 'AI toggle has three stars');
    assert.equal(await page.locator('[data-action="open-summary"]').innerText(), 'Niki와 대화');
    const refreshCalls = () => page.evaluate(() => calls.filter(c => c.name === 'codex_refresh_memory').length);
    const refreshBase = await refreshCalls();
    await page.evaluate(() => { memoryLeft = 3; });
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('.codex-memory-status').waitFor();
    await page.waitForFunction(base => calls.filter(c => c.name === 'codex_refresh_memory').length >= base + 3, refreshBase);
    await page.locator('.codex-memory-status').waitFor({ state: 'detached' });
    assert.equal(await refreshCalls(), refreshBase + 3, 'background memory building stops once nothing is left');
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_chat').length), 0);
    assert.equal(await page.locator('[data-testid="codex-summary-result"]').count(), 1, 'reopening retains summary');
    await page.getByRole('textbox', { name: 'Niki에게 메시지', exact: true }).fill('이 이야기를 더 해 볼까?');
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();
    await page.locator('[data-action="open-summary"]').click();
    assert.equal(await page.getByRole('textbox', { name: 'Niki에게 메시지', exact: true }).inputValue(), '이 이야기를 더 해 볼까?', 'closing retains draft');
    await page.getByRole('button', { name: '메시지 보내기' }).click();
    await page.locator('[data-testid="codex-chat-result"]').waitFor();
    assert.deepEqual(await page.evaluate(() => calls.find(c => c.name === 'codex_chat').args), { message: '이 이야기를 더 해 볼까?' }, 'chat does not silently transmit the current diary');
    const interruptBase = await refreshCalls();
    await page.evaluate(() => { memoryLeft = 8; });
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('.codex-memory-status').waitFor();
    await page.getByRole('textbox', { name: 'Niki에게 메시지', exact: true }).fill('정리 중에 보내는 메시지');
    await page.getByRole('button', { name: '메시지 보내기' }).click();
    await page.waitForFunction(() => calls.filter(c => c.name === 'codex_chat').length >= 2);
    assert.ok(await page.evaluate(() => memoryLeftAtChat > 0), 'sending a message interrupts background memory building instead of waiting for it to finish');
    assert.ok(interruptBase >= 0);
    // Compared with the count taken inside the chat call, so a fast machine cannot finish early.
    await page.waitForFunction(() => calls.filter(c => c.name === 'codex_refresh_memory').length > refreshCallsAtChat);
    assert.ok(true, 'memory building resumes after a reply');
    // The search model arrives automatically: no button, only progress while it downloads.
    assert.equal(await page.getByRole('button', { name: /검색 모델/ }).count(), 0, 'no download button');
    assert.equal(await page.locator('[data-testid="memory-model-status"]').count(), 0, 'nothing shown once installed');
    await page.evaluate(() => Object.assign(memoryModel, { installed: false, downloading: true, downloaded_bytes: 55910939 }));
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();
    await page.locator('[data-action="open-summary"]').click();
    await page.getByText('일기 검색을 준비하고 있어요… 50%', { exact: true }).waitFor();
    const beforeInstall = await refreshCalls();
    await page.evaluate(() => Object.assign(memoryModel, { installed: true, downloading: false, downloaded_bytes: memoryModel.total_bytes }));
    await page.locator('[data-testid="memory-model-status"]').waitFor({ state: 'detached' });
    await page.waitForFunction(base => calls.filter(c => c.name === 'codex_refresh_memory').length > base, beforeInstall);
    await page.evaluate(() => Object.assign(memoryModel, { installed: false, downloading: false, failed: true }));
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();
    await page.locator('[data-action="open-summary"]').click();
    await page.getByText('지금은 단어로 일기를 찾아요', { exact: false }).waitFor();
    await page.evaluate(() => Object.assign(memoryModel, { installed: true, failed: false }));
    await page.locator('[data-testid="memory-model-status"]').waitFor({ state: 'detached' });
    await page.evaluate(() => { failSave = true; });
    await page.locator('#entry-title').fill('충돌한 편집');
    await page.locator('[data-action="codex-summarize"]').click();
    await page.waitForFunction(() => calls.filter(c => c.name === 'save_entry_checked').length >= 2);
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_summarize').length), 1, 'failed save must block transmission');
    await page.evaluate(() => { failSave = false; deferSummary = true; });
    await page.locator('#entry-title').fill('저장 충돌 해결');
    await page.waitForFunction(() => sample.entries[0].title === '저장 충돌 해결');
    await page.locator('[data-action="codex-summarize"]').click();
    await page.waitForFunction(() => typeof completeSummary === 'function');
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();
    await page.evaluate(() => location.hash = '#/journals/j2/entries/e2');
    await page.waitForFunction(() => document.querySelector('#entry-title')?.value === '다른 일기');
    await page.evaluate(() => completeSummary('첫 일기 전용 요약'));
    await page.locator('[data-action="open-summary"]').click();
    await page.getByText('첫 일기 전용 요약', { exact: true }).waitFor();
    assert.equal(await page.locator('[data-testid="codex-summary-result"]').count(), 2, 'late summary is retained in shared conversation across journals');
    await page.evaluate(() => { deferSummary = false; });
    await page.locator('[data-action="codex-summarize"]').click();
    await page.waitForFunction(() => chatHistory.filter(m => m.kind === 'summary' && m.role === 'assistant').length === 3);
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_summarize').at(-1).args.entryId), 'e2', 'summary targets the currently open diary');
    await page.evaluate(() => { failChat = true; });
    await page.getByRole('textbox', { name: 'Niki에게 메시지', exact: true }).fill('전송 오류 테스트');
    await page.getByRole('button', { name: '메시지 보내기' }).click();
    await page.locator('#diary-summary-sidebar [role="alert"]').waitFor();
    assert.equal(await page.getByRole('textbox', { name: 'Niki에게 메시지', exact: true }).inputValue(), '전송 오류 테스트');
    assert.equal(await page.locator('[data-testid="codex-summary-result"]').count(), 3, 'failure retains existing history');
    const savedHistory = await page.evaluate(() => chatHistory);
    await page.addInitScript(history => { window.chatHistory = history; window.codexStatus.connected = true; }, savedHistory);
    await page.reload();
    await page.locator('[data-action="open-summary"]').click();
    await page.getByText('첫 일기 전용 요약', { exact: true }).waitFor();
    assert.equal(await page.locator('[data-testid="codex-summary-result"]').count(), 3, 'reload restores backend history');
    await page.evaluate(() => location.hash = '#/calendar');
    await page.locator('.calendar').waitFor();
    assert.equal(await page.locator('[data-testid="codex-summary-result"]').count(), 3, 'navigation retains global conversation');
    assert.ok(await page.locator('[data-action="codex-summarize"]').isDisabled(), 'summary requires an open diary');
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();
    await page.evaluate(() => location.hash = '#/settings/codex');
    await page.locator('[data-action="codex-disconnect"]').click();
    await page.locator('[data-action="codex-connect"]').waitFor();
    await page.evaluate(() => location.hash = '#/all/entries/e1');
    await page.locator('#entry-title').waitFor();
    assert.equal(await page.locator('[data-action="open-summary"]').count(), 0);
    assert.deepEqual(errors, []);
    console.log('Codex login, shared persistent chat, current-diary summaries, save conflicts, text-only output and responsive UI tests passed');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
