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
    page.on('pageerror', error => errors.push(error.message));
    await page.route('https://**/*', route => route.abort());
    await page.addInitScript(() => {
      window.calls = [];
      const entry = id => ({ id, journal_id: 'j', title: `일기 ${id}`, body: `오늘 ${id} 산책을 했다.`, date: '2026-11-10', created_at: 'v1', updated_at: 'v1', tags: [], favorite: false, attachments: [], location: '', weather: '', deleted_at: null });
      window.sample = { locked: false, app_version: '0.1.4', nickname: '', updater_config: {}, journals: [{ id: 'j', name: '일기장', color: '#576b61' }], entries: [entry('a'), entry('b')], templates: [], sync_conflicts: [], pending_resolution_count: 0, synced_entry_ids: [], sync_status: {} };
      window.connection = { accounts: [], active: null, model: 'gpt-6-luna', pending_registration: true };
      window.holdSummary = false; window.holdLogin = false; window.summaryError = ''; window.failSave = false;
      window.__TAURI__ = { core: { invoke: async (name, args) => {
        calls.push({ name, args });
        if (name === 'bootstrap') return structuredClone(sample);
        if (name === 'chatgpt_status') return structuredClone(connection);
        if (name === 'chatgpt_open_usage') return;
        if (name === 'chatgpt_connect') {
          if (holdLogin) await new Promise((resolve, reject) => { window.rejectLogin = reject; });
          const id = args.clientId || `oaiapp_${connection.accounts.length + 1}`;
          let account = connection.accounts.find(a => a.client_id === id);
          if (!account) { account = { client_id: id, email: 'person@example.invalid', welcome_seen: false }; connection.accounts.push(account); }
          Object.assign(account, { connected: true, plan_enabled: true }); connection.active = id; connection.pending_registration = false;
          return structuredClone(connection);
        }
        if (name === 'chatgpt_acknowledge') { connection.accounts.find(a => a.client_id === connection.active).welcome_seen = true; return; }
        if (name === 'chatgpt_select_account') { connection.active = args.clientId; return structuredClone(connection); }
        if (name === 'chatgpt_disconnect') { Object.assign(connection.accounts.find(a => a.client_id === connection.active), { connected: false, plan_enabled: false }); return '이 기기에서 로그아웃했습니다. 원격 세션 해제를 확인하지 못했으므로 ChatGPT 설정에서도 Niki 연결을 해제해 주세요.'; }
        if (name === 'chatgpt_cancel') { if (window.rejectLogin) { rejectLogin(Error('요청을 취소했습니다')); window.rejectLogin = null; } return; }
        if (name === 'chatgpt_summarize') {
          if (holdSummary) await new Promise(resolve => { window.releaseSummary = resolve; });
          if (summaryError) throw Error(summaryError);
          return { text: '산책을 하며 하루를 보냈다. <img src=x onerror=alert(1)>', model: 'gpt-6-luna', input_tokens: 100, output_tokens: 20 };
        }
        if (name === 'save_entry_checked') {
          if (failSave) throw Error('일기 저장 실패');
          const e = sample.entries.find(e => e.id === args.input.id);
          const assertRevision = JSON.stringify(e) === JSON.stringify(args.input.expectedEntry);
          if (!assertRevision) throw Error('일기가 변경되었습니다');
          Object.assign(e, { title: args.input.title, body: args.input.body, updated_at: `saved-${calls.length}` });
          return { id: e.id, entry: structuredClone(e) };
        }
        throw Error(`Unexpected command ${name}`);
      } } };
    });
    const action = name => page.locator(`[data-action="${name}"]`);
    const screenshot = async name => {
      if (!process.env.NIKI_CHATGPT_SCREENSHOTS) return;
      fs.mkdirSync(process.env.NIKI_CHATGPT_SCREENSHOTS, { recursive: true });
      await page.screenshot({ path: path.join(process.env.NIKI_CHATGPT_SCREENSHOTS, `${name}.png`) });
    };
    await page.goto(pathToFileURL(path.resolve('ui/index.html')).href);
    await action('entry').first().click();
    await page.locator('.ProseMirror').waitFor();
    await action('summarize-entry').click();
    await action('summary-connect').waitFor();
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_summarize').length), 0);
    await action('summary-connect').click();
    await action('chatgpt-connect').waitFor();
    assert.equal(await page.locator('[data-oai-siwc-button]').innerText(), 'Continue with ChatGPT');
    assert.equal(await page.locator('[data-oai-siwc-button]').evaluate(e => getComputedStyle(e).backgroundColor), 'rgb(17, 17, 17)');
    await page.evaluate(() => { holdLogin = true; });
    await action('chatgpt-connect').click();
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_connect').at(-1).args.newRegistration), false, 'Normal retries retain the pending registration');
    await action('chatgpt-cancel-login').click();
    await page.waitForFunction(() => calls.some(c => c.name === 'chatgpt_cancel'));
    await page.getByRole('alert').filter({ hasText: '취소' }).waitFor();
    await page.evaluate(() => { holdLogin = false; });
    await action('chatgpt-add-account').click();
    assert.deepEqual(await page.evaluate(() => { const a = calls.filter(c => c.name === 'chatgpt_connect').at(-1).args; return {clientId:a.clientId,newRegistration:a.newRegistration}; }), {clientId:null,newRegistration:true});
    await action('chatgpt-welcome-ok').click();
    await page.waitForFunction(() => connection.accounts[0].welcome_seen);
    await action('chatgpt-add-account').click();
    await action('chatgpt-welcome-ok').click();
    await page.getByLabel('사용할 ChatGPT 계정', { exact: true }).selectOption('oaiapp_1');
    await page.waitForFunction(() => connection.active === 'oaiapp_1');
    assert.equal(await page.locator('dialog[open]').count(), 0);
    await action('chatgpt-usage').click();
    await screenshot('connected-settings');
    await action('all').click();
    await action('entry').first().click();
    const id = await page.locator('[data-action="entry"].active').getAttribute('data-id');
    await page.locator('#entry-title').fill('저장한 최신 제목');
    await action('summarize-entry').click();
    await page.waitForFunction(() => document.querySelector('[data-action="summary-start"]')?.disabled === false);
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_summarize').length), 0, 'Consent must precede any network summary');
    const before = await page.evaluate(() => structuredClone(sample.entries));
    assert.equal(before.find(e => e.id === id).title, '저장한 최신 제목');
    await action('summary-start').click();
    await page.getByRole('region', { name: '일기 요약 결과' }).waitFor();
    const request = await page.evaluate(() => calls.find(c => c.name === 'chatgpt_summarize').args);
    assert.equal(request.entryId, id);
    assert.equal(request.expectedUpdatedAt, before.find(e => e.id === id).updated_at);
    assert.deepEqual(Object.keys(request).sort(), ['entryId', 'expectedUpdatedAt', 'requestId']);
    assert.deepEqual(await page.evaluate(() => sample.entries), before, 'Summaries must not modify diaries');
    assert.equal(await page.locator('.chatgpt-summary-result img').count(), 0, 'AI output is plain text');
    assert.ok((await page.locator('.chatgpt-summary-result').innerText()).includes('<img'));
    await screenshot('diary-summary');
    await action('summary-close').click();
    await page.evaluate(() => { summaryError = 'ChatGPT 사용량 한도에 도달했습니다. 사용량 관리에서 확인해 주세요.'; });
    await action('summarize-entry').click(); await action('summary-start').click();
    await page.getByRole('alert').filter({ hasText: '사용량 한도' }).waitFor();
    await action('summary-usage').click();
    await action('summary-close').click();
    await page.evaluate(() => { summaryError = ''; holdSummary = true; });
    await action('summarize-entry').click(); await action('summary-start').click();
    await page.waitForFunction(() => typeof releaseSummary === 'function');
    await action('summary-close').click();
    const cancelledId = await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_cancel').at(-1).args.requestId);
    assert.equal(cancelledId, await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_summarize').at(-1).args.requestId));
    await page.locator(`[data-action="entry"]:not([data-id="${id}"])`).click();
    await page.evaluate(() => { releaseSummary(); holdSummary = false; });
    await page.waitForTimeout(100);
    assert.equal(await page.locator('dialog[open]').count(), 0, 'Cancelled results must not reappear on another diary');
    await page.evaluate(() => { failSave = true; });
    await page.locator('#entry-title').fill('저장 실패 제목');
    const previousCount = await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_summarize').length);
    await action('summarize-entry').click();
    await page.waitForFunction(() => document.querySelector('#save-status').textContent.includes('실패'));
    assert.equal(await page.locator('dialog[open]').count(), 0);
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_summarize').length), previousCount);
    await page.evaluate(() => { failSave = false; });
    for (const width of [900, 1280, 1600]) {
      await page.setViewportSize({ width, height: 800 });
      const overflow = await page.locator('.editor-pane').evaluate(e => e.scrollWidth - e.clientWidth);
      assert.equal(overflow, 0, `Editor overflows at ${width}px`);
    }
    await action('settings').click();
    await page.locator('#settings-detail').waitFor();
    await page.locator('[data-action="settings-tab"]').filter({ hasText: 'ChatGPT 연결' }).click();
    await action('chatgpt-disconnect').click();
    await page.getByText('이 기기에서 로그아웃했습니다.', { exact: false }).waitFor();
    await action('chatgpt-connect').click();
    await action('chatgpt-disconnect').waitFor();
    assert.equal(await page.locator('dialog[open]').count(), 0, 'Returning sign-in must not repeat the welcome');
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'chatgpt_connect').at(-1).args.clientId), 'oaiapp_1');
    assert.deepEqual(errors, []);
    console.log('ChatGPT settings, consent, account switching, cancellation, summary privacy, error recovery and responsive UI passed.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
