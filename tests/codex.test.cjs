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
      window.calls = []; window.failSave = false; window.deferSummary = false;
      window.codexStatus = { installed: true, connected: false, plan: null, login_pending: false };
      const entry = (id, title, body) => ({ id, journal_id: 'j1', title, body, date: '2026-11-10', created_at: 'v1', updated_at: 'v1', tags: [], favorite: false, location: '', weather: '', attachments: [], deleted_at: null });
      window.sample = {
        locked: false, nickname: '나', app_version: '0.1.22', updater_config: {},
        journals: [{ id: 'j1', name: '매일 일기', color: '#576b61' }],
        entries: [entry('e1', '산책', '공원에서 산책했다.'), entry('e2', '다른 일기', '다른 비공개 내용')],
        templates: [], sync_conflicts: [], pending_resolution_count: 0, synced_entry_ids: [],
        sync_status: { connected: false, sync_paused: false, pending_upload_count: 0 },
      };
      window.__TAURI__ = { core: { invoke: async (name, args) => {
        calls.push({ name, args });
        if (name === 'bootstrap') return structuredClone(sample);
        if (name === 'codex_status') return structuredClone(codexStatus);
        if (name === 'codex_connect') { codexStatus.login_pending = true; return { verification_url: 'https://auth.openai.com/codex/device', user_code: 'DEMO-CODE' }; }
        if (name === 'codex_disconnect') { Object.assign(codexStatus, { connected: false, plan: null, login_pending: false }); return; }
        if (name === 'save_entry_checked') {
          if (failSave) throw Error('다른 기기에서 이 일기가 변경되었습니다');
          const current = sample.entries.find(e => e.id === args.input.id);
          Object.assign(current, { title: args.input.title, body: args.input.body, updated_at: 'v2' });
          return { id: current.id, entry: structuredClone(current) };
        }
        if (name === 'codex_summarize') {
          if (deferSummary) return new Promise(resolve => { window.completeSummary = resolve; });
          return '<script>window.injected = true</script> 산책하며 편안한 하루를 보냈다.';
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
    await page.locator('[data-action="open-summary"]').waitFor({ state: 'hidden' });
    assert.equal(await page.locator('#diary-summary-sidebar').count(), 0, 'disconnect must remove sidebar and its contents');
    await page.evaluate(() => location.hash = '#/settings/codex');
    await page.locator('[data-action="codex-connect"]').click();
    assert.equal(await page.getByRole('heading', { name: 'GPT 계정 연결', exact: true }).count(), 1);
    assert.equal(await page.getByText('선택한 일기의 제목과 본문이 OpenAI에 전송됩니다.', { exact: true }).count(), 1);
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
    assert.equal(await page.locator('#diary-summary-sidebar input, #diary-summary-sidebar textarea').count(), 0, 'sidebar currently shows summaries only');
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
        const panel = document.querySelector('#diary-summary-sidebar').getBoundingClientRect();
        return { editorRight: editor.right, editorWidth: editor.width, panelLeft: panel.left, panelRight: panel.right, width: innerWidth, overflow: document.documentElement.scrollWidth - innerWidth };
      });
      assert.ok(layout.editorRight <= layout.panelLeft + 1 && layout.editorWidth >= 300, JSON.stringify(layout));
      assert.ok(layout.panelRight <= layout.width && layout.overflow <= 0, JSON.stringify(layout));
    };
    await checkSidebar();
    await page.setViewportSize({ width: 900, height: 650 });
    await checkSidebar();
    await page.screenshot({ path: 'output/codex-ui/summary-narrow.png' });
    await page.getByRole('button', { name: '요약 닫기', exact: true }).click();
    assert.ok(await page.locator('.list-pane').isVisible(), 'closing summary restores diary list');
    await page.locator('[data-action="open-summary"]').click();
    await page.evaluate(() => { failSave = true; });
    await page.locator('#entry-title').fill('충돌한 편집');
    await page.locator('[data-action="codex-summarize"]').click();
    await page.waitForFunction(() => calls.filter(c => c.name === 'save_entry_checked').length >= 2);
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_summarize').length), 1, 'failed save must block transmission');
    await page.evaluate(() => { failSave = false; deferSummary = true; });
    await page.locator('[data-action="codex-summarize"]').click();
    await page.waitForFunction(() => typeof completeSummary === 'function');
    await page.evaluate(() => location.hash = '#/all/entries/e2');
    await page.locator('#entry-title').waitFor();
    await page.evaluate(() => completeSummary('첫 일기 전용 요약'));
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('[data-action="codex-summarize"]').waitFor();
    assert.equal(await page.locator('[data-testid="codex-summary-result"]').count(), 0, 'late summary must not leak into another diary');
    await page.evaluate(() => location.hash = '#/settings/codex');
    await page.locator('[data-action="codex-disconnect"]').click();
    await page.locator('[data-action="codex-connect"]').waitFor();
    await page.evaluate(() => location.hash = '#/all/entries/e1');
    await page.locator('#entry-title').waitFor();
    assert.equal(await page.locator('[data-action="open-summary"]').count(), 0);
    assert.deepEqual(errors, []);
    console.log('Codex login, explicit consent, saved selection, conflicts, text-only output, privacy and responsive UI tests passed');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
