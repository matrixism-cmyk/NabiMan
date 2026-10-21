// Browser smoke test for talking with Claude by voice.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_VOICE_URL=http://127.0.0.1:19999 \
//   SMOKE_VOICE_LOG=/tmp/fake-voice.log npm run smoke:claude
//
// Needs a NabiMan server serving build/ that can run Claude Code, and a
// stand-in speech service (the scratch fake_voice2.py, started with
// FAKE_WORDS="한 단어로만 답해 사과는 무슨 색이야", with or without a final 엔터) that "hears" that
// sentence, reads aloud with a tone, and logs what it was sent.
const fs = require('fs');
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const VOICE_URL = process.env.SMOKE_VOICE_URL || 'http://127.0.0.1:19999';
const VOICE_KEY = process.env.SMOKE_VOICE_KEY || 'vk_test_dictation_key';
const VOICE_LOG = process.env.SMOKE_VOICE_LOG;

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};
const api = (page, path, init = {}) => page.evaluate(async ({ path, init }) => {
  const res = await fetch(path, { ...init, headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${sessionStorage.getItem('nabiman_token')}` } });
  return res.json();
}, { path, init });
const logged = () => (VOICE_LOG && fs.existsSync(VOICE_LOG) ? fs.readFileSync(VOICE_LOG, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);

(async () => {
  const browser = await chromium.launch({ args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required'] });
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await context.grantPermissions(['microphone'], { origin: BASE });
  const page = await context.newPage();
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  await api(page, '/api/terminal/dictation/settings', { method: 'PUT', body: JSON.stringify({ url: VOICE_URL, address: '', key: VOICE_KEY }) });
  const current = (await api(page, '/api/terminal/claude/settings')).data.settings;
  await api(page, '/api/terminal/claude/settings', { method: 'PUT', body: JSON.stringify({ ...current, speed: 1, continuous: true, speaker: 'sohee', tools: 'none' }) });
  const before = logged().length;

  // 1. The Claude menu, at the right end.
  const cats = await page.locator('.cat-nav .cat-btn').allTextContents();
  check('Claude is the last main menu', cats[cats.length - 1].trim() === 'Claude', cats.join(' / '));
  await page.locator('.cat-btn', { hasText: 'Claude' }).click();
  await page.locator('.cv').waitFor();

  // 2. 새 채팅 listens at once; "엔터" sends what was heard.
  await page.locator('.cv-welcome button', { hasText: '새 채팅' }).click();
  await page.locator('.cv-state.is-listening').waitFor({ timeout: 15000 }).catch(() => {});
  check('새 채팅 starts listening', await page.locator('.cv-state.is-listening').count() === 1);
  await page.locator('.cv-msg.is-user').waitFor({ timeout: 30000 }).catch(() => {});
  const asked = await page.locator('.cv-msg.is-user .cv-bubble').first().textContent().catch(() => '');
  check('what was said is sent (on "엔터" or a pause), without the word', asked.includes('사과는 무슨 색이야') && !asked.includes('엔터'), asked);
  await page.locator('.cv-state.is-thinking').waitFor({ timeout: 10000 }).catch(() => {});
  await page.waitForFunction(() => document.querySelector('.cv-state')?.classList.contains('is-speaking'), null, { timeout: 60000 }).catch(() => {});
  check('the answer is read aloud as it comes', await page.locator('.cv-state.is-speaking').count() === 1);
  await page.screenshot({ path: `${OUT}/smoke-claude-speaking.png` });
  await page.waitForFunction(() => document.querySelector('.cv-state')?.classList.contains('is-listening')
    && !document.querySelector('.cv-state')?.classList.contains('is-speaking')
    && !document.querySelector('.cv-state')?.classList.contains('is-thinking'), null, { timeout: 90000 }).catch(() => {});
  const answer = await page.locator('.cv-msg.is-assistant .cv-bubble').last().textContent().catch(() => '');
  check('Claude answers', /빨|붉|red|초록|녹/i.test(answer), answer.slice(0, 60));
  check('then listening goes on for the next question', await page.locator('.cv-state.is-listening').count() === 1);
  check('nothing more was sent while it was read (the microphone rested)', await page.locator('.cv-msg.is-user').count() === 1);
  const spoken = logged().slice(before).filter((e) => e.event === 'tts');
  check('the speech service read the answer, keeping nothing', spoken.length > 0 && spoken.every((e) => e.save === false && e.speaker === 'sohee'), `${spoken.length} sentence(s)`);

  // 3. Typing works too, and the conversation goes on from the last answer.
  await page.locator('.cv-mic').click();
  await page.locator('.cv-draft').fill('방금 내가 물어본 과일 이름만 말해.');
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => document.querySelectorAll('.cv-msg.is-assistant').length >= 2
    && !document.querySelector('.cv-state.is-thinking'), null, { timeout: 90000 }).catch(() => {});
  const second = await page.locator('.cv-msg.is-assistant .cv-bubble').last().textContent().catch(() => '');
  check('a typed question goes on from the last (same session)', second.includes('사과'), second.slice(0, 40));
  await page.locator('.cv-stop').click({ timeout: 2000 }).catch(() => {});
  const listed = await page.locator('.cv-chat-title').allTextContents();
  check('the conversation is listed by what was asked', listed.some((x) => x.includes('사과는 무슨 색이야')), listed.join(' / '));

  // 4. Settings: voice, speed and manner are kept; 들어보기 uses the unsaved choice.
  await page.locator('.sidebar-btn', { hasText: '설정' }).click();
  await page.locator('.cv-settings').waitFor();
  await page.locator('.cv-settings select').first().selectOption('aiden');
  await page.locator('.cv-settings input[type="range"]').fill('1.5');
  await page.locator('.cv-settings input[placeholder*="차분"]').fill('밝게');
  const n = logged().length;
  await page.locator('.cv-settings button', { hasText: '들어보기' }).click();
  await page.waitForTimeout(1500);
  const tried = logged().slice(n).filter((e) => e.event === 'tts');
  check('들어보기 reads in the chosen voice', tried.length === 1 && tried[0].speaker === 'aiden', JSON.stringify(tried[0] || {}));
  await page.locator('.cv-settings button', { hasText: '저장' }).click();
  await page.locator('.cv-settings .message', { hasText: '저장했습니다' }).waitFor({ timeout: 5000 }).catch(() => {});
  const saved = (await api(page, '/api/terminal/claude/settings')).data.settings;
  check('settings are saved', saved.speaker === 'aiden' && saved.speed === 1.5 && saved.instruct === '밝게', JSON.stringify(saved).slice(0, 120));
  await page.locator('.cv-settings button', { hasText: 'Claude 실행 확인' }).click();
  await page.locator('.cv-settings .message', { hasText: 'Claude Code' }).waitFor({ timeout: 30000 }).catch(() => {});
  check('Claude 실행 확인 shows the version and account', await page.locator('.cv-settings .message', { hasText: 'Claude Code' }).count() === 1);
  await page.screenshot({ path: `${OUT}/smoke-claude-settings.png`, fullPage: true });
  await api(page, '/api/terminal/claude/settings', { method: 'PUT', body: JSON.stringify({ ...saved, speaker: 'sohee', speed: 1, instruct: '' }) });

  check('no page errors', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
