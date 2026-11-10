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
      const now = new Date();
      const prefix = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}`;
      window.testDate = `${prefix}-10`;
      const makeEntry = (id, date, title, journal = 'j1', deleted = null) => ({ id, journal_id: journal, title, body: '', date, created_at: 'v1', updated_at: 'v1', tags: [], favorite: false, location: '', weather: '', attachments: [], deleted_at: deleted });
      window.sample = {
        locked: false, nickname: '나', updater_config: {},
        journals: [{ id: 'j1', name: '매일 일기', color: '#576b61' }, { id: 'j2', name: '여행 일기장', color: '#ae815f' }],
        entries: [
          ...['아침 산책', '점심에 만난 친구', '오후의 작은 발견', '긴 제목이 있는 일기도 날짜 칸의 너비를 넘어가지 않도록 표시합니다', '하루를 마무리하며'].map((title, index) => makeEntry(`a${index}`, window.testDate, title)),
          ...['책을 읽은 아침', '잠깐의 휴식', '오늘의 감사'].map((title, index) => makeEntry(`b${index}`, `${prefix}-11`, title)),
          makeEntry('other', window.testDate, '다른 일기장의 일기', 'j2'),
          makeEntry('deleted', window.testDate, '삭제한 일기', 'j1', 'deleted'),
        ],
        templates: [], sync_conflicts: [], pending_resolution_count: 0, synced_entry_ids: [],
        sync_status: { connected: false, sync_paused: false, pending_upload_count: 0 },
      };
      window.__TAURI__ = { core: { invoke: async name => {
        if (name === 'bootstrap') return structuredClone(sample);
        throw Error(`Unexpected command ${name}`);
      } } };
    });
    await page.goto(pathToFileURL(path.resolve('ui/index.html')).href);
    await page.locator('#editor-pane .empty [data-action="new-entry"]').waitFor();
    const gap = await page.evaluate(() => document.querySelector('#editor-pane .empty [data-action="new-entry"]').getBoundingClientRect().top - document.querySelector('#editor-pane .empty p').getBoundingClientRect().bottom);
    assert.ok(gap >= 16, `New diary button gap is only ${gap}px`);
    const screenshot = async name => {
      if (!process.env.NIKI_CALENDAR_SCREENSHOTS) return;
      fs.mkdirSync(process.env.NIKI_CALENDAR_SCREENSHOTS, { recursive: true });
      await page.screenshot({ path: path.join(process.env.NIKI_CALENDAR_SCREENSHOTS, `${name}.png`) });
    };
    await screenshot('empty-spacing');
    await page.locator('[data-action="journal"][data-id="j1"]').click();
    await page.waitForFunction(() => location.hash === '#/journals/j1');
    await page.locator('[data-action="calendar"]').click();
    await page.locator('.calendar').waitFor();
    const date = await page.evaluate(() => testDate);
    const cell = page.locator(`.day[data-date="${date}"]`);
    const neighbor = page.locator(`.day[data-date="${date.slice(0, -2)}11"]`);
    const more = cell.locator('[data-action="calendar-more"]');
    assert.equal(await cell.locator('em').count(), 2);
    assert.equal(await more.innerText(), '+3개 더보기');
    assert.equal(await neighbor.locator('em').count(), 2);
    assert.equal(await page.locator('button button').count(), 0, 'Interactive buttons must not be nested');
    for (const width of [900, 1280, 1600]) {
      await page.setViewportSize({ width, height: 800 });
      const layout = await page.evaluate(() => {
        const days = [...document.querySelectorAll('.calendar .day')];
        const calendar = document.querySelector('.calendar').getBoundingClientRect();
        const header = document.querySelector('.wide-head').getBoundingClientRect();
        return { aligned: Math.abs(calendar.right - header.right) < 1, overflow: document.querySelector('.wide-view').scrollWidth - document.querySelector('.wide-view').clientWidth,
          topLeft: days.every(day => { const box = day.getBoundingClientRect(), number = day.querySelector('.day-number').getBoundingClientRect(); return number.top - box.top < 20 && number.left - box.left < 15; }) };
      });
      assert.deepEqual(layout, { aligned: true, overflow: 0, topLeft: true }, `Calendar layout at ${width}px`);
    }
    await page.setViewportSize({ width: 1280, height: 800 });
    await screenshot('calendar-collapsed');
    const collapsed = (await cell.boundingBox()).height;
    const route = new URL(page.url()).hash;
    await more.click();
    assert.equal(await cell.locator('em').count(), 5);
    assert.equal(await neighbor.locator('em').count(), 2, 'Expanding one date must not expand another');
    assert.equal(await more.getAttribute('aria-expanded'), 'true');
    assert.equal(new URL(page.url()).hash, route, 'More must not navigate away');
    assert.ok((await cell.boundingBox()).height > collapsed, 'Expanded entries must grow the calendar cell');
    await screenshot('calendar-expanded');
    await more.focus();
    await page.keyboard.press('Enter');
    assert.equal(await cell.locator('em').count(), 2);
    assert.equal(await more.getAttribute('aria-expanded'), 'false');
    assert.ok(Math.abs((await cell.boundingBox()).height - collapsed) < 1);
    await more.click();
    await page.locator('[data-action="next-month"]').click();
    await page.locator('[data-action="prev-month"]').click();
    assert.equal(await cell.locator('em').count(), 2, 'Month navigation resets expanded dates');
    await cell.locator('[data-action="calendar-day"]').click();
    await page.waitForFunction(date => location.hash === `#/journals/j1?date=${date}`, date);
    assert.deepEqual(errors, []);
    console.log('Calendar previews, expand/collapse, filtering, keyboard, date navigation and responsive layout passed.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
