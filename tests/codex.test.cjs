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
    await page.goto(pathToFileURL(path.resolve('ui/index.html')).href + '#/settings/codex');
    await page.locator('[data-action="codex-connect"]').click();
    await page.locator('.codex-device-code').waitFor();
    assert.equal(await page.locator('.codex-device-code').innerText(), 'DEMO-CODE');
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_open_login').length), 0, 'login must not open a browser automatically');
    await page.evaluate(() => Object.assign(codexStatus, { connected: true, plan: 'free', login_pending: false }));
    await page.getByText('연결됨 · free').waitFor();
    assert.equal(await page.locator('.codex-device-code').count(), 0);
    await page.evaluate(() => location.hash = '#/all/entries/e1');
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('[data-action="codex-summarize"]').waitFor();
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
    await page.setViewportSize({ width: 900, height: 650 });
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'summary must fit minimum window width');
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
    assert.deepEqual(errors, []);
    console.log('Codex login, explicit consent, saved selection, conflicts, text-only output, privacy and responsive UI tests passed');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
