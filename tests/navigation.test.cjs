const { chromium } = require('playwright-core');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', args: ['--allow-file-access-from-files'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    window.calls = [];
    window.sample = {
      locked: false, lock_enabled: false, app_version: '0.1.9', installed_on_pc: false, nickname: '나', updater_config: {},
      journals: [{ id: 'j1', name: '첫 일기장', color: '#576b61' }, { id: 'j2', name: '둘째 일기장', color: '#ae815f' }],
      entries: [{ id: 'e1', journal_id: 'j1', title: '기록', body: '본문', date: '2026-10-29', created_at: 'v1', updated_at: 'v1', tags: [], favorite: false, location: '', weather: '', attachments: [], deleted_at: null }],
      templates: [], sync_conflicts: [], pending_resolution_count: 0, synced_entry_ids: [], sync_status: { connected: false, has_oauth_client: true, sync_paused: false, pending_upload_count: 0 },
    };
    window.__TAURI__ = { core: { invoke: async (name, args) => {
      calls.push({ name, args });
      if (name === 'bootstrap') return structuredClone(sample);
      if (name === 'save_entry') { const entry = { ...sample.entries[0], ...args.input, id: 'e2', journal_id: args.input.journalId, updated_at: 'v2' }; sample.entries.push(entry); return entry.id; }
      if (name === 'save_entry_checked') { const entry = sample.entries.find(item => item.id === args.input.id); Object.assign(entry, { title: args.input.title, body: args.input.body, date: args.input.date, journal_id: args.input.journalId, favorite: args.input.favorite, tags: args.input.tags, location: args.input.location, weather: args.input.weather, updated_at: 'v3' }); return { id: entry.id, entry: structuredClone(entry) }; }
      if (name === 'trash_entry') { sample.entries.find(entry => entry.id === args.id).deleted_at = 'now'; return; }
      if (name === 'restore_entry') { sample.entries.find(entry => entry.id === args.id).deleted_at = null; return; }
      if (name === 'drive_connect') { Object.assign(sample.sync_status, { connected: true, account_email: 'niki@example.com', account_name: 'Niki' }); return; }
      if (name === 'drive_sync_now') return { warnings: [] };
      if (name === 'drive_set_paused') { sample.sync_status.sync_paused = args.paused; return; }
      if (name === 'drive_disconnect') { sample.sync_status.connected = false; return; }
      throw Error(name);
    } } };
  });
  await page.goto(pathToFileURL(path.resolve('ui/index.html')).href);
  await page.locator('[data-action="journal"][data-id="j2"]').click();
  assert.equal(new URL(page.url()).hash, '#/journals/j2');
  await page.locator('[data-action="calendar"]').click();
  assert.equal(new URL(page.url()).hash, '#/calendar?journal=j2');
  await page.locator('[data-action="media"]').click();
  assert.equal(new URL(page.url()).hash, '#/media?journal=j2');
  await page.locator('[data-action="new-entry"]').first().click();
  await page.waitForSelector('#entry-title');
  assert.equal(await page.evaluate(() => calls.find(call => call.name === 'save_entry').args.input.journalId), 'j2');
  assert.equal(new URL(page.url()).hash, '#/journals/j2/entries/e2');
  await page.locator('[data-action="date-picker"]').click();
  assert.equal(await page.locator('[role="dialog"][aria-label="날짜 선택"]').count(), 1);
  await page.locator('#date-direct').fill('2026-10-28');
  await page.waitForFunction(() => sample.entries.find(entry => entry.id === 'e2').date === '2026-10-28');
  await page.locator('[data-action="entry-menu"]').click();
  assert.equal(await page.evaluate(() => calls.filter(call => call.name === 'trash_entry').length), 0, 'opening menu must not delete');
  assert.equal(await page.locator('[data-action="trash-entry"]').count(), 1);
  page.once('dialog', dialog => dialog.accept());
  await page.locator('[data-action="trash-entry"]').click();
  await page.waitForFunction(() => sample.entries.find(entry => entry.id === 'e2').deleted_at);
  await page.locator('[data-action="trash"]').click();
  await page.locator('[data-action="restore"]').click();
  await page.waitForFunction(() => !sample.entries.find(entry => entry.id === 'e2').deleted_at);
  await page.locator('[data-action="settings"]').click();
  await page.waitForFunction(() => location.hash === '#/settings/sync');
  assert.equal(new URL(page.url()).hash, '#/settings/sync');
  await page.locator('[data-action="settings-tab"]').filter({ hasText: '앱 정보' }).click();
  assert.equal(new URL(page.url()).hash, '#/settings/about');
  await page.waitForFunction(() => document.querySelector('#settings-detail h3')?.textContent === 'Niki v0.1.9');
  assert.equal(await page.locator('#settings-detail h3').first().innerText(), 'Niki v0.1.9');
  await page.locator('[data-action="settings-tab"]').filter({ hasText: 'Google Drive' }).click();
  await page.locator('[data-action="google-login"]').click();
  await page.waitForFunction(() => document.querySelector('.sync-badge')?.textContent === '자동 동기화');
  assert.equal(await page.locator('.account-sub').innerText(), 'niki@example.com');
  await page.locator('[data-action="drive-toggle-pause"]').click();
  await page.waitForFunction(() => document.querySelector('.sync-badge')?.textContent === '일시 중지');
  await page.locator('[data-action="drive-disconnect"]').click();
  await page.waitForFunction(() => !document.querySelector('.sync-badge'));
  assert.deepEqual(errors, []);
  await browser.close();
  console.log('React routing, editor controls, trash and settings tests passed');
})().catch(error => { console.error(error); process.exit(1); });
