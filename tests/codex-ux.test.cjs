const { chromium } = require('playwright-core');
const assert = require('node:assert/strict');
const { pathToFileURL } = require('node:url');
const path = require('node:path');
const fs = require('node:fs');

// Chat experience and settings: consent, sources, safety card, progress and stop, composer,
// starters, answer actions, pinning, hiding journals, and the Codex settings cards.
(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', args: ['--allow-file-access-from-files'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    await page.route('https://**/*', route => route.abort());
    await page.addInitScript(() => {
      window.calls = [];
      window.chatHistory = [];
      window.holdChat = false;
      window.prefs = { consented: false, excluded_journals: [], model: null, default_model: 'gpt-6-luna', model_fallback: false, background: true };
      window.cli = { path: '/Users/me/.local/bin/codex', version: '0.167.1', supported: true, custom: false, min_version: '0.167' };
      window.memoryInfo = { model_installed: true, model_bytes: 111822000, index_bytes: 2048000, indexed_entries: 1, total_entries: 2, downloading: false, failed: false, months_summarized: 3, chat_summarized: true };
      window.listeners = {};
      const entry = (id, title, body, journal = 'j1') => ({ id, journal_id: journal, title, body, date: '2026-11-10', created_at: 'v1', updated_at: 'v1', tags: [], favorite: false, location: '', weather: '', attachments: [], deleted_at: null });
      window.sample = {
        locked: false, nickname: '나', app_version: '0.1.28', updater_config: {},
        journals: [{ id: 'j1', name: '매일 일기', color: '#576b61' }, { id: 'j2', name: '여행 일기', color: '#576b61' }],
        entries: [entry('e1', '산책', '공원에서 산책했다.'), entry('e2', '제주 여행', '제주도 바닷가를 걸었다.', 'j2')],
        templates: [], sync_conflicts: [], pending_resolution_count: 0, synced_entry_ids: [],
        sync_status: { connected: true, sync_paused: false, pending_upload_count: 0 },
      };
      window.emitProgress = payload => (listeners['niki-chat-progress'] || []).forEach(cb => cb({ payload }));
      window.__TAURI__ = {
        event: { listen: async (name, cb) => { (listeners[name] ||= []).push(cb); return () => {}; } },
        core: { invoke: async (name, args) => {
          calls.push({ name, args });
          switch (name) {
            case 'bootstrap': return structuredClone(sample);
            case 'drive_sync_now': return { warnings: [] };
            case 'codex_status': return { installed: true, connected: true, plan: 'plus', login_pending: false };
            case 'codex_conversation': return structuredClone(chatHistory);
            case 'codex_prefs': return structuredClone(prefs);
            case 'codex_set_consent': prefs.consented = args.consented; return structuredClone(prefs);
            case 'codex_set_journal_excluded':
              prefs.excluded_journals = prefs.excluded_journals.filter(id => id !== args.journalId);
              if (args.excluded) prefs.excluded_journals.push(args.journalId);
              return structuredClone(prefs);
            case 'codex_set_prefs': Object.assign(prefs, { model: args.model, model_fallback: args.modelFallback, background: args.background }); return structuredClone(prefs);
            case 'codex_models': return [{ id: 'gpt-6-luna', name: 'GPT-6 Luna' }, { id: 'gpt-6-sol', name: 'GPT-6 Sol' }, { id: 'long-model', name: 'Long model name '.repeat(20) }];
            case 'codex_cli_info': return structuredClone(cli);
            case 'codex_set_cli_path':
              if (args.path === '/bad') throw Error('이 파일은 Codex CLI가 아닌 것 같습니다.');
              Object.assign(cli, args.path ? { path: args.path, custom: true } : { path: '/Users/me/.local/bin/codex', custom: false });
              return structuredClone(cli);
            case 'codex_memory_model': return { installed: true, downloading: false, failed: false, downloaded_bytes: 0, total_bytes: 111821878 };
            case 'codex_memory_info': return structuredClone(memoryInfo);
            case 'codex_memory_reset': return;
            case 'codex_refresh_memory': return { remaining: 0 };
            case 'codex_cancel': window.cancelRequested = true; return;
            case 'codex_clear_conversation': chatHistory.length = 0; return [];
            case 'codex_retract_last': {
              const answer = chatHistory.pop(); const question = chatHistory.pop();
              return { text: question.content, history: structuredClone(chatHistory) };
            }
            case 'codex_chat': {
              if (holdChat) {
                await new Promise((resolve, reject) => { window.finishChat = resolve; const t = setInterval(() => { if (window.cancelRequested) { clearInterval(t); window.cancelRequested = false; reject('cancelled'); } }, 30); });
              }
              chatHistory.push({ id: `m${chatHistory.length}`, role: 'user', kind: 'chat', content: args.message });
              chatHistory.push({ id: `m${chatHistory.length}`, role: 'assistant', kind: 'chat', content: '제주도에서 행복했던 기억이 있었네요.\n그 마음을 천천히 떠올려 봐요.',
                sources: [{ id: 'e2', journal_id: 'j2', date: '2026-11-10', title: '제주 여행' }, { id: 'gone', journal_id: 'j1', date: '2025-01-01', title: '지워진 일기' }] });
              return structuredClone(chatHistory);
            }
            default: throw Error(`Unexpected command: ${name}`);
          }
        } },
      };
    });
    await page.goto(pathToFileURL(path.resolve('ui/index.html')).href + '#/all/entries/e1');
    await page.locator('#entry-title').waitFor();
    const composer = page.getByRole('textbox', { name: 'Niki에게 메시지', exact: true });
    const callCount = name => page.evaluate(n => calls.filter(c => c.name === n).length, name);

    // 3 (consent): nothing can be sent until the user agrees; agreeing is recorded.
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('[data-testid="consent-card"]').waitFor();
    assert.equal(await composer.count(), 0, 'no composer before consent');
    assert.ok((await page.locator('[data-testid="consent-card"]').innerText()).includes('OpenAI'));
    assert.equal(await page.locator('[data-action="codex-starter-summary"]').count(), 0, 'no summary action before consent');
    await page.locator('[data-action="codex-consent"]').click();
    await composer.waitFor();
    assert.deepEqual(await page.evaluate(() => calls.find(c => c.name === 'codex_set_consent').args), { consented: true });

    fs.mkdirSync('output/codex-ui', { recursive: true });
    // Empty composer and starter icons must not overflow or inherit hero-icon spacing.
    for (const width of [1280, 900]) {
      await page.setViewportSize({ width, height: 650 });
      const actions = await page.locator('.topbar [data-slot="button"]').evaluateAll(buttons => buttons.map(button => {
        const rect = button.getBoundingClientRect();
        return { action: button.dataset.action, y: rect.y, right: rect.right };
      }));
      assert.deepEqual(actions.map(button => button.action), ['drive-sync', 'open-summary', 'new-entry']);
      assert.ok(actions.every(button => Math.abs(button.y - actions[0].y) < 1 && button.right <= width), JSON.stringify(actions));
      assert.equal(await page.locator('.codex-summary-head [data-action="codex-summarize"]').count(), 0);
      const layout = await page.evaluate(() => {
        const field = document.querySelector('.codex-input');
        const icon = document.querySelector('.codex-starter svg');
        const panel = document.querySelector('.codex-summary');
        return { fieldOverflow: field.scrollHeight - field.clientHeight,
          iconMargin: getComputedStyle(icon).marginBottom,
          panelOverflow: panel.scrollWidth - panel.clientWidth,
          bodyOverflow: document.documentElement.scrollWidth - innerWidth };
      });
      assert.ok(layout.fieldOverflow <= 1, JSON.stringify(layout));
      assert.equal(layout.iconMargin, '0px');
      assert.ok(layout.panelOverflow <= 1 && layout.bodyOverflow <= 0, JSON.stringify(layout));
      assert.equal(await page.locator('[data-action="codex-new-chat"]').innerText(), '새 대화');
      await page.screenshot({ path: `output/codex-ui/chat-empty-${width}.png` });
    }
    await page.setViewportSize({ width: 1280, height: 800 });

    // 7 (starters) and 14 (not synced note).
    assert.ok((await page.locator('#diary-summary-sidebar').innerText()).includes('다른 기기와 동기화되지 않아요'));
    assert.equal(await page.locator('[data-action="codex-starter-summary"]').count(), 1, 'summary starter while a diary is open');
    await page.locator('[data-action="codex-starter"]').first().click();
    await page.locator('[data-testid="codex-chat-result"]').waitFor();
    assert.equal(await page.evaluate(() => calls.find(c => c.name === 'codex_chat').args.message), '요즘 내 감정 흐름은 어때?');

    // 1 (sources): links to the diaries that were looked up; a deleted one is disabled.
    const chips = page.locator('[data-action="codex-source"]');
    assert.equal(await chips.count(), 2);
    assert.ok(await chips.nth(1).isDisabled(), 'unknown entry cannot be opened');
    await chips.nth(0).click();
    await page.waitForFunction(() => location.hash.includes('/journals/j2/entries/e2'));
    await page.waitForFunction(() => document.querySelector('#entry-title')?.value === '제주 여행');

    // 8 (answer actions): add the answer to the open diary; ask again.
    await page.locator('[data-action="codex-add-to-diary"]').click();
    await page.waitForFunction(() => document.querySelector('[aria-label="일기 본문"]')?.innerText.includes('Niki의 이야기'));
    assert.ok((await page.locator('[aria-label="일기 본문"]').innerText()).includes('행복했던 기억'));
    await page.locator('[data-action="codex-regenerate"]').click();
    await page.waitForFunction(() => calls.filter(c => c.name === 'codex_chat').length >= 2);
    assert.equal(await callCount('codex_retract_last'), 1);
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_chat')[1].args.message), '요즘 내 감정 흐름은 어때?', 'asks the same question again');
    await page.locator('[data-testid="codex-chat-result"]').waitFor();
    assert.equal(await page.locator('[data-testid="codex-chat-result"]').count(), 1, 'the old answer was replaced');

    // 6 (composer): Shift+Enter breaks the line, Enter sends.
    await composer.fill('첫 줄');
    await composer.press('Shift+Enter');
    await composer.type('둘째 줄');
    assert.equal(await composer.inputValue(), '첫 줄\n둘째 줄');
    await composer.press('Enter');
    await page.waitForFunction(() => calls.filter(c => c.name === 'codex_chat').length >= 3);
    assert.equal(await page.evaluate(() => calls.filter(c => c.name === 'codex_chat')[2].args.message), '첫 줄\n둘째 줄');

    // 4 (safety): the card appears in the app itself, with real numbers.
    await composer.fill('요즘 너무 힘들어서 죽고 싶어');
    await composer.press('Enter');
    await page.waitForFunction(() => calls.filter(c => c.name === 'codex_chat').length >= 4);
    await page.locator('[data-testid="crisis-card"]').first().waitFor();
    const card = await page.locator('[data-testid="crisis-card"]').first().innerText();
    assert.ok(card.includes('109') && card.includes('1577-0199') && card.includes('119'), card);
    assert.equal(await page.locator('[data-testid="crisis-card"]').count(), 1, 'only the worrying message gets a card');

    // 5 (progress and stop): steps, streamed text, and the stop button restoring the question.
    await page.evaluate(() => { holdChat = true; });
    await composer.fill('천천히 답해 줘');
    await composer.press('Enter');
    await page.locator('[data-testid="codex-stage"]').waitFor();
    assert.ok((await page.locator('[data-testid="codex-stage"]').innerText()).includes('찾고 있어요'));
    await page.evaluate(() => emitProgress({ stage: 'writing', text: null }));
    await page.waitForFunction(() => document.querySelector('[data-testid="codex-stage"]')?.innerText.includes('답변을 쓰고 있어요'));
    await page.evaluate(() => emitProgress({ stage: 'writing', text: '지금까지 쓴 답변' }));
    await page.locator('[data-testid="codex-streaming"]').waitFor();
    assert.equal(await page.locator('[data-testid="codex-streaming"]').innerText(), '지금까지 쓴 답변');
    await page.locator('[data-action="codex-cancel"]').click();
    await page.waitForFunction(() => !document.querySelector('[data-action="codex-cancel"]'));
    assert.equal(await callCount('codex_cancel'), 1);
    assert.equal(await composer.inputValue(), '천천히 답해 줘', 'a stopped question returns to the box');
    assert.equal(await page.locator('#diary-summary-sidebar .sync-error').count(), 0, 'stopping is not an error');
    await page.evaluate(() => { holdChat = false; });

    // 2 (new conversation): asks first, then clears.
    await page.locator('[data-action="codex-new-chat"]').click();
    await page.getByRole('button', { name: '새로 시작', exact: true }).click();
    await page.waitForFunction(() => calls.some(c => c.name === 'codex_clear_conversation'));
    await page.waitForFunction(() => !document.querySelector('[data-testid="codex-chat-result"]'));

    // 9 (pin): the diary moves aside instead of being covered, and the choice is remembered.
    const editorWidth = () => page.evaluate(() => document.querySelector('.editor-document').getBoundingClientRect().width);
    const overlaid = await editorWidth();
    await page.locator('[data-action="codex-pin"]').click();
    await page.waitForFunction(() => document.querySelector('.content.chat-pinned'));
    assert.ok(await editorWidth() < overlaid, 'pinned panel shares the row with the diary');
    const pinnedLayout = await page.evaluate(() => ({
      editor: document.querySelector('.editor-document').getBoundingClientRect().right,
      chat: document.querySelector('.codex-summary').getBoundingClientRect().left,
    }));
    assert.ok(pinnedLayout.editor <= pinnedLayout.chat + 1, JSON.stringify(pinnedLayout));
    await page.setViewportSize({ width: 900, height: 600 });
    assert.ok(await page.locator('[data-action="codex-pin"]').isVisible());
    assert.ok(await page.getByRole('button', { name: '메시지 보내기', exact: true }).isVisible());
    await page.setViewportSize({ width: 1280, height: 800 });
    assert.equal(await page.evaluate(() => localStorage.getItem('niki-chat-pinned')), '1');
    await page.locator('[data-action="codex-pin"]').click();
    await page.waitForFunction(() => !document.querySelector('.content.chat-pinned'));
    await page.getByRole('button', { name: '대화 닫기', exact: true }).click();

    await page.waitForFunction(() => getComputedStyle(document.querySelector('#diary-summary-sidebar')).visibility === 'hidden');

    // 3 (hide a journal): from the sidebar menu; shown by an icon; reversible.
    await page.locator('[data-action="journal"][data-id="j2"]').click({ button: 'right' });
    await page.locator('[data-action="toggle-journal-hidden"]').click();
    await page.waitForFunction(() => document.querySelector('[data-action="journal"][data-id="j2"] .journal-hidden'));
    assert.deepEqual(await page.evaluate(() => calls.filter(c => c.name === 'codex_set_journal_excluded').at(-1).args), { journalId: 'j2', excluded: true });

    // Settings: 10 (Codex CLI), 11 (model), 12 (background), 13 (memory), and consent.
    await page.evaluate(() => location.hash = '#/settings/codex');
    assert.equal(await page.locator('#settings-detail h2').innerText(), 'AI 연결');
    assert.ok(!await page.locator('[data-testid="cli-card"]').isVisible(), 'technical details start collapsed');
    await page.screenshot({ path: 'output/codex-ui/ai-settings.png' });
    await page.locator('[data-testid="advanced-settings"] > summary').click();
    await page.locator('[data-testid="cli-card"]').waitFor();
    const cliText = await page.locator('[data-testid="cli-card"]').innerText();
    assert.ok(cliText.includes('0.167.1') && cliText.includes('/Users/me/.local/bin/codex') && cliText.includes('자동 감지'), cliText);
    await page.locator('[data-testid="cli-card"] summary').click();
    await page.getByRole('textbox', { name: 'Codex 실행 파일 전체 경로' }).fill('/bad');
    await page.locator('[data-action="codex-set-path"]').click();
    await page.locator('[data-testid="cli-card"] [role="alert"]').waitFor();
    await page.getByRole('textbox', { name: 'Codex 실행 파일 전체 경로' }).fill('/opt/codex/bin/codex');
    await page.locator('[data-action="codex-set-path"]').click();
    await page.getByText('/opt/codex/bin/codex (직접 지정)').waitFor();
    await page.locator('[data-action="codex-clear-path"]').click();
    await page.getByText('(자동 감지)').waitFor();

    await page.locator('[data-action="codex-load-models"]').click();
    await page.getByRole('option', { name: 'GPT-6 Sol' }).waitFor({ state: 'attached' });
    await page.getByLabel('Niki가 쓸 모델').selectOption('gpt-6-sol');
    await page.getByLabel(/다른 AI로 답하기/).check();
    await page.getByLabel(/지난 일기와 긴 대화 자동 요약/).uncheck();
    await page.locator('[data-action="codex-save-prefs"]').click();
    await page.getByText('저장했어요.').waitFor();
    assert.deepEqual(await page.evaluate(() => calls.filter(c => c.name === 'codex_set_prefs').at(-1).args), { model: 'gpt-6-sol', modelFallback: true, background: false });

    await page.getByLabel('Niki가 쓸 모델').selectOption('long-model');
    for (const width of [1280, 900]) {
      await page.setViewportSize({ width, height: 650 });
      const layout = await page.evaluate(() => {
        const detail = document.querySelector('#settings-detail');
        const check = document.querySelector('.codex-check input').getBoundingClientRect();
        return { overflow: detail.scrollWidth - detail.clientWidth, checkbox: check.width,
          bodyOverflow: document.documentElement.scrollWidth - innerWidth };
      });
      assert.equal(layout.checkbox, 16, JSON.stringify(layout));
      assert.ok(layout.overflow <= 1 && layout.bodyOverflow <= 0, JSON.stringify(layout));
      await page.locator('[data-testid="usage-card"]').scrollIntoViewIfNeeded();
      await page.screenshot({ path: `output/codex-ui/ai-advanced-${width}.png` });
    }
    await page.setViewportSize({ width: 1280, height: 800 });

    const memoryText = await page.locator('[data-testid="memory-card"]').innerText();
    assert.ok(memoryText.includes('1/2편') && memoryText.includes('월별 요약 3개') && memoryText.includes('설치됨'), memoryText);
    assert.equal(await page.locator('[data-testid="memory-card"] [role="progressbar"]').getAttribute('aria-valuenow'), '50');
    await page.locator('[data-action="codex-reset-index"]').click();
    await page.getByRole('button', { name: '지우기', exact: true }).click();
    await page.waitForFunction(() => calls.some(c => c.name === 'codex_memory_reset'));
    assert.deepEqual(await page.evaluate(() => calls.find(c => c.name === 'codex_memory_reset').args), { what: 'index' });

    const privacy = await page.locator('[data-testid="privacy-card"]').innerText();
    assert.ok(privacy.includes('동의했어요') && privacy.includes('여행 일기') && privacy.includes('동기화되지 않아요'), privacy);
    await page.locator('[data-action="codex-unhide-journal"]').click();
    await page.waitForFunction(() => !document.querySelector('[data-action="codex-unhide-journal"]'));
    await page.locator('[data-action="codex-revoke-consent"]').click();
    await page.getByRole('button', { name: '동의 거두기', exact: true }).last().click();
    await page.getByText('아직 동의하지 않았어요').waitFor();

    // Withdrawn consent brings the consent card back in the chat.
    await page.evaluate(() => location.hash = '#/all/entries/e1');
    await page.locator('[data-action="open-summary"]').click();
    await page.locator('[data-testid="consent-card"]').waitFor();
    assert.deepEqual(errors, []);
    console.log('Consent, sources, safety card, progress and stop, composer, starters, answer actions, pinning, hidden journals and Codex settings tests passed');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
