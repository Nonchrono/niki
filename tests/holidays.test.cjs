const { chromium } = require('playwright-core');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe',
    args: ['--allow-file-access-from-files']
  });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));

  await page.addInitScript(() => {
    window.calls = [];
    window.sample = {
      locked: false, lock_enabled: false, app_version: '0.1.9', installed_on_pc: false, nickname: '나', updater_config: {},
      journals: [{ id: 'j1', name: '첫 일기장', color: '#576b61' }],
      entries: [{ id: 'e1', journal_id: 'j1', title: '현충일 일기', body: '기록', date: '2026-06-06', created_at: 'v1', updated_at: 'v1', tags: [], favorite: false, location: '', weather: '', attachments: [], deleted_at: null }],
      templates: [], sync_conflicts: [], pending_resolution_count: 0, synced_entry_ids: [],
      sync_status: { connected: false, has_oauth_client: true, sync_paused: false, pending_upload_count: 0 },
    };
    window.__TAURI__ = {
      core: {
        invoke: async (name, args) => {
          if (name === 'bootstrap') return structuredClone(sample);
          if (name === 'save_entry') {
            const entry = { ...sample.entries[0], ...args.input, id: 'e2', updated_at: 'v2' };
            sample.entries.push(entry);
            return entry.id;
          }
          if (name === 'save_entry_checked') {
            return { id: args.input.id, entry: structuredClone(args.input) };
          }
          return null;
        }
      }
    };
  });

  await page.goto(pathToFileURL(path.resolve('ui/index.html')).href);
  await page.locator('[data-action="calendar"]').waitFor();
  await page.locator('[data-action="calendar"]').click();
  await page.waitForSelector('.calendar');

  // 1. 요일 헤더 색상 검증
  const sundayHeaderColor = await page.evaluate(() => {
    const el = document.querySelector('.weekday.sunday');
    return el ? getComputedStyle(el).color : null;
  });
  const saturdayHeaderColor = await page.evaluate(() => {
    const el = document.querySelector('.weekday.saturday');
    return el ? getComputedStyle(el).color : null;
  });
  assert.equal(sundayHeaderColor, 'rgb(220, 38, 38)', '일요일 헤더는 빨간색이어야 합니다');
  assert.equal(saturdayHeaderColor, 'rgb(37, 99, 235)', '토요일 헤더는 파란색이어야 합니다');

  // 2. 2026년 6월로 이동 (2026-06-06은 토요일이면서 현충일 공휴일!)
  // 현재 헤더의 연/월 확인
  let currentHeader = await page.locator('.wide-head h2').innerText();
  // 2026년 6월이 될 때까지 이전/다음 버튼 클릭
  while (!currentHeader.includes('2026년 6월')) {
    const match = currentHeader.match(/(\d+)년\s*(\d+)월/);
    if (!match) break;
    const y = parseInt(match[1], 10);
    const m = parseInt(match[2], 10);
    if (y > 2026 || (y === 2026 && m > 6)) {
      await page.locator('[data-action="prev-month"]').click();
    } else {
      await page.locator('[data-action="next-month"]').click();
    }
    await page.waitForTimeout(50);
    currentHeader = await page.locator('.wide-head h2').innerText();
  }

  // 2026년 6월 6일(토요일 & 현충일 공휴일) 검증
  const june6thInfo = await page.evaluate(() => {
    const days = Array.from(document.querySelectorAll('.calendar .day'));
    for (const d of days) {
      const numSpan = d.querySelector('.day-number');
      const isOutside = d.classList.contains('outside');
      if (numSpan && numSpan.textContent.trim() === '6' && !isOutside) {
        const holidayBadge = d.querySelector('.day-holiday');
        return {
          classList: Array.from(d.classList),
          numberColor: getComputedStyle(numSpan).color,
          cellBorderColor: getComputedStyle(d).borderColor,
          holidayText: holidayBadge ? holidayBadge.textContent.trim() : null,
          hasHolidayClass: d.classList.contains('holiday'),
          hasSaturdayClass: d.classList.contains('saturday'),
        };
      }
    }
    return null;
  });

  assert.ok(june6thInfo, '2026년 6월 6일 날짜 칸을 찾을 수 있어야 합니다');
  assert.equal(june6thInfo.hasHolidayClass, true, '토요일과 공휴일이 겹치면 holiday 클래스가 적용되어야 합니다');
  assert.equal(june6thInfo.numberColor, 'rgb(220, 38, 38)', '토요일과 공휴일이 겹치면 일자 숫자는 빨간색이어야 합니다');
  assert.equal(june6thInfo.holidayText, '현충일', '공휴일 배지에 현충일이 표시되어야 합니다');

  // 2026년 6월 13일(일반 토요일, 공휴일 아님) 검증
  const june13thInfo = await page.evaluate(() => {
    const days = Array.from(document.querySelectorAll('.calendar .day'));
    for (const d of days) {
      const numSpan = d.querySelector('.day-number');
      const isOutside = d.classList.contains('outside');
      if (numSpan && numSpan.textContent.trim() === '13' && !isOutside) {
        return {
          numberColor: getComputedStyle(numSpan).color,
          hasSaturdayClass: d.classList.contains('saturday'),
          hasHolidayClass: d.classList.contains('holiday'),
        };
      }
    }
    return null;
  });

  assert.ok(june13thInfo, '2026년 6월 13일 날짜 칸을 찾을 수 있어야 합니다');
  assert.equal(june13thInfo.hasSaturdayClass, true, '토요일에는 saturday 클래스가 적용되어야 합니다');
  assert.equal(june13thInfo.numberColor, 'rgb(37, 99, 235)', '일반 토요일의 일자 숫자는 파란색이어야 합니다');

  // 2026년 6월 7일(일반 일요일) 검증
  const june7thInfo = await page.evaluate(() => {
    const days = Array.from(document.querySelectorAll('.calendar .day'));
    for (const d of days) {
      const numSpan = d.querySelector('.day-number');
      const isOutside = d.classList.contains('outside');
      if (numSpan && numSpan.textContent.trim() === '7' && !isOutside) {
        return {
          numberColor: getComputedStyle(numSpan).color,
          hasSundayClass: d.classList.contains('sunday'),
        };
      }
    }
    return null;
  });

  assert.ok(june7thInfo, '2026년 6월 7일 날짜 칸을 찾을 수 있어야 합니다');
  assert.equal(june7thInfo.hasSundayClass, true, '일요일에는 sunday 클래스가 적용되어야 합니다');
  assert.equal(june7thInfo.numberColor, 'rgb(220, 38, 38)', '일요일의 일자 숫자는 빨간색이어야 합니다');

  // 3. DatePicker 팝오버 달력 검증
  // 에디터 페이지로 이동하여 DatePicker 테스트
  await page.locator('[data-action="all"]').click();
  await page.locator('[data-action="new-entry"]').first().click();
  await page.locator('[data-action="date-picker"]').waitFor();
  await page.locator('[data-action="date-picker"]').click();
  await page.locator('.date-popover').waitFor();

  // DatePicker 요일 헤더 색상 검증
  const datePickerSundayColor = await page.evaluate(() => {
    const el = document.querySelector('.date-weekday.sunday');
    return el ? getComputedStyle(el).color : null;
  });
  const datePickerSaturdayColor = await page.evaluate(() => {
    const el = document.querySelector('.date-weekday.saturday');
    return el ? getComputedStyle(el).color : null;
  });
  assert.equal(datePickerSundayColor, 'rgb(220, 38, 38)', 'DatePicker 일요일 헤더는 빨간색이어야 합니다');
  assert.equal(datePickerSaturdayColor, 'rgb(37, 99, 235)', 'DatePicker 토요일 헤더는 파란색이어야 합니다');

  // DatePicker 헤더를 2026년 6월로 이동
  let dpHeader = await page.locator('.date-header strong').innerText();
  while (!dpHeader.includes('2026년 6월')) {
    const match = dpHeader.match(/(\d+)년\s*(\d+)월/);
    if (!match) break;
    const y = parseInt(match[1], 10);
    const m = parseInt(match[2], 10);
    if (y > 2026 || (y === 2026 && m > 6)) {
      await page.locator('[data-action="date-prev"]').click();
    } else {
      await page.locator('[data-action="date-next"]').click();
    }
    await page.waitForTimeout(50);
    dpHeader = await page.locator('.date-header strong').innerText();
  }

  // DatePicker 2026년 6월 6일(토요일 & 현충일 공휴일) 검증: 빨간색
  const dpJune6thColor = await page.evaluate(() => {
    const btns = Array.from(document.querySelectorAll('.date-grid .date-day'));
    for (const b of btns) {
      if (b.textContent.trim() === '6' && !b.classList.contains('outside')) {
        return {
          color: getComputedStyle(b).color,
          hasHoliday: b.classList.contains('holiday'),
        };
      }
    }
    return null;
  });
  assert.ok(dpJune6thColor, 'DatePicker 6월 6일 버튼을 찾을 수 있어야 합니다');
  assert.equal(dpJune6thColor.hasHoliday, true, 'DatePicker 6월 6일에는 holiday 클래스가 있어야 합니다');
  assert.equal(dpJune6thColor.color, 'rgb(220, 38, 38)', 'DatePicker 6월 6일(토요일+공휴일)은 빨간색이어야 합니다');

  // DatePicker 2026년 6월 13일(일반 토요일) 검증: 파란색
  const dpJune13thColor = await page.evaluate(() => {
    const btns = Array.from(document.querySelectorAll('.date-grid .date-day'));
    for (const b of btns) {
      if (b.textContent.trim() === '13' && !b.classList.contains('outside')) {
        return {
          color: getComputedStyle(b).color,
          hasSaturday: b.classList.contains('saturday'),
        };
      }
    }
    return null;
  });
  assert.ok(dpJune13thColor, 'DatePicker 6월 13일 버튼을 찾을 수 있어야 합니다');
  assert.equal(dpJune13thColor.hasSaturday, true, 'DatePicker 6월 13일에는 saturday 클래스가 있어야 합니다');
  assert.equal(dpJune13thColor.color, 'rgb(37, 99, 235)', 'DatePicker 일반 토요일은 파란색이어야 합니다');

  // DatePicker 2026년 6월 7일(일반 일요일) 검증: 빨간색
  const dpJune7thColor = await page.evaluate(() => {
    const btns = Array.from(document.querySelectorAll('.date-grid .date-day'));
    for (const b of btns) {
      if (b.textContent.trim() === '7' && !b.classList.contains('outside')) {
        return {
          color: getComputedStyle(b).color,
          hasSunday: b.classList.contains('sunday'),
        };
      }
    }
    return null;
  });
  assert.ok(dpJune7thColor, 'DatePicker 6월 7일 버튼을 찾을 수 있어야 합니다');
  assert.equal(dpJune7thColor.hasSunday, true, 'DatePicker 6월 7일에는 sunday 클래스가 있어야 합니다');
  assert.equal(dpJune7thColor.color, 'rgb(220, 38, 38)', 'DatePicker 일반 일요일은 빨간색이어야 합니다');

  console.log('달력 및 공휴일 색상 검증 테스트 모두 통과!');
  await browser.close();
  if (errors.length > 0) {
    console.error('Page errors encountered:', errors);
    process.exit(1);
  }
})();
