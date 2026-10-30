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
  await page.locator('#search').waitFor();
  const listLayout = await page.evaluate(() => {
    const search = document.querySelector('#search');
    const icon = document.querySelector('.list-search > span');
    const favorite = document.querySelector('[data-action="favorites"]');
    const tag = document.querySelector('#tag-filter');
    const textStart = search.getBoundingClientRect().left + parseFloat(getComputedStyle(search).paddingLeft);
    return { searchGap: textStart - icon.getBoundingClientRect().right, controlHeightGap: Math.abs(favorite.getBoundingClientRect().height - tag.getBoundingClientRect().height) };
  });
  assert.ok(listLayout.searchGap >= 8, `search icon overlaps text (${listLayout.searchGap}px)`);
  assert.ok(listLayout.controlHeightGap <= 1, `filter control heights differ (${listLayout.controlHeightGap}px)`);
  assert.equal(await page.locator('[data-action="new-entry"][data-slot="button"]').count() > 0, true, 'shadcn buttons should render');
  assert.equal(await page.locator('[data-action="calendar"] svg.lucide').count(), 1, 'Lucide navigation icon should render');
  assert.equal(await page.evaluate(() => getComputedStyle(document.body).backgroundColor), 'rgb(248, 248, 245)', 'Tailwind theme should be loaded');
  await page.locator('[data-action="journal"][data-id="j2"]').click();
  await page.waitForFunction(() => location.hash === '#/journals/j2');
  assert.equal(new URL(page.url()).hash, '#/journals/j2');
  await page.locator('[data-action="calendar"]').click();
  await page.waitForFunction(() => location.hash === '#/calendar?journal=j2');
  assert.equal(new URL(page.url()).hash, '#/calendar?journal=j2');
  await page.locator('[data-action="media"]').click();
  await page.waitForFunction(() => location.hash === '#/media?journal=j2');
  assert.equal(new URL(page.url()).hash, '#/media?journal=j2');
  await page.locator('[data-action="new-entry"]').first().click();
  await page.waitForSelector('#entry-title');
  assert.equal(await page.evaluate(() => calls.find(call => call.name === 'save_entry').args.input.journalId), 'j2');
  assert.equal(new URL(page.url()).hash, '#/journals/j2/entries/e2');
  await page.locator('[data-action="date-picker"]').click();
  assert.equal(await page.locator('#entry-journal[data-slot="native-select"]').count(), 1, 'shadcn native select should render');
  assert.equal(await page.locator('[data-action="format"][data-format="bold"] svg.lucide').count(), 1, 'Lucide editor icon should render');
  const editorHeights = await page.evaluate(() => ['[data-action="date-picker"]', '#entry-journal', '#entry-tags']
    .map(selector => document.querySelector(selector).getBoundingClientRect().height));
  assert.ok(Math.max(...editorHeights) - Math.min(...editorHeights) <= 1, `entry metadata control heights differ (${editorHeights})`);
  assert.equal(await page.locator('[role="dialog"][aria-label="날짜 선택"]').count(), 1);
  await page.locator('#date-direct').fill('2026-10-28');
  await page.waitForFunction(() => sample.entries.find(entry => entry.id === 'e2').date === '2026-10-28');
  if (process.env.NIKI_SCREENSHOT) await page.screenshot({ path: process.env.NIKI_SCREENSHOT, fullPage: true });
  await page.setViewportSize({ width: 1024, height: 768 });
  const editorOverflow = await page.evaluate(() => ({ page: document.documentElement.scrollWidth - innerWidth,
    toolbar: document.querySelector('.toolbar').scrollWidth - document.querySelector('.toolbar').clientWidth }));
  assert.ok(editorOverflow.page <= 0 && editorOverflow.toolbar <= 0, `editor overflows at 1024px (${JSON.stringify(editorOverflow)})`);
  await page.setViewportSize({ width: 1280, height: 800 });
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
  await page.locator('[data-action="settings-tab"]').filter({ hasText: '템플릿' }).click();
  await page.locator('#settings-detail [data-action="new-template"]').waitFor();
  const templateGap = await page.evaluate(() => document.querySelector('.settings-card > [data-slot="button"]').getBoundingClientRect().top
    - document.querySelector('.settings-card > p').getBoundingClientRect().bottom);
  assert.ok(templateGap >= 10, `settings description and action are too close (${templateGap}px)`);
  if (process.env.NIKI_SCREENSHOT_SETTINGS) await page.screenshot({ path: process.env.NIKI_SCREENSHOT_SETTINGS, fullPage: true });
  await page.locator('[data-action="settings-tab"]').filter({ hasText: '앱 정보' }).click();
  await page.waitForFunction(() => location.hash === '#/settings/about');
  assert.equal(new URL(page.url()).hash, '#/settings/about');
  await page.waitForFunction(() => document.querySelector('#settings-detail h3')?.textContent === 'Niki v0.1.9');
  assert.equal(await page.locator('#settings-detail h3').first().innerText(), 'Niki v0.1.9');
  await page.locator('[data-action="settings-tab"]').filter({ hasText: 'Google Drive' }).click();
  await page.locator('[data-action="google-login"]').click();
  await page.waitForFunction(() => document.querySelector('.sync-badge')?.textContent === '자동 동기화');
  assert.equal(await page.locator('.account-sub').innerText(), 'niki@example.com');
  const syncButtonHeights = await page.evaluate(() => ['[data-action="google-login"]', '[data-action="drive-disconnect"]', '#settings-detail [data-action="drive-sync"]', '[data-action="drive-toggle-pause"]']
    .map(selector => document.querySelector(selector).getBoundingClientRect().height));
  assert.ok(Math.max(...syncButtonHeights) - Math.min(...syncButtonHeights) <= 1, `settings action heights differ (${syncButtonHeights})`);
  if (process.env.NIKI_SCREENSHOT_SYNC) await page.screenshot({ path: process.env.NIKI_SCREENSHOT_SYNC, fullPage: true });
  await page.locator('[data-action="drive-toggle-pause"]').click();
  await page.waitForFunction(() => document.querySelector('.sync-badge')?.textContent === '일시 중지');
  await page.locator('[data-action="drive-disconnect"]').click();
  await page.waitForFunction(() => !document.querySelector('.sync-badge'));
  assert.deepEqual(errors, []);
  await browser.close();
  console.log('React routing, editor controls, trash and settings tests passed');
})().catch(error => { console.error(error); process.exit(1); });
