// Browser smoke test for previewing video, audio, archives and other files.
//
//   SMOKE_URL=http://127.0.0.1:18096 SMOKE_MEDIA_DIR=/tmp/media-files npm run smoke:media
//
// Needs a NabiMan server serving build/ and a folder on this machine (the
// local shell's files) holding: clip.webm (VP8/Vorbis, 640×360, 12 s),
// clip-h264.mp4, tone.ogg, tone.wav, song.mp3, pack.zip (readme.txt,
// 문서/보고서.txt, docs/), tool (an ELF binary), settings.myconf (text with an
// unknown extension) and empty.dat (empty). A player must fetch the file a
// range at a time, never whole.
const { chromium } = require('playwright');

const BASE = process.env.SMOKE_URL || 'http://127.0.0.1:18096';
const PASSWORD = process.env.SMOKE_PASSWORD || 'smoketest123';
const OUT = process.env.SMOKE_OUT || '/tmp';
const DIR = process.env.SMOKE_MEDIA_DIR;
if (!DIR) { console.log('FATAL: set SMOKE_MEDIA_DIR'); process.exit(1); }

const results = [];
const check = (name, ok, detail = '') => {
  results.push(ok);
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${detail ? ` — ${detail}` : ''}`);
};

(async () => {
  const browser = await chromium.launch({ args: ['--autoplay-policy=no-user-gesture-required'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const problems = [];
  page.on('pageerror', (e) => problems.push(e.message.split('\n')[0]));
  page.on('dialog', (d) => { problems.push(`browser dialog: ${d.message()}`); d.dismiss(); });
  const streamed = [];
  page.on('request', (r) => { if (r.url().includes('/stream?')) streamed.push({ url: r.url(), range: r.headers().range || '' }); });

  await page.goto(BASE, { waitUntil: 'networkidle' });
  await page.fill('input[type="text"]', 'admin');
  await page.fill('input[type="password"]', PASSWORD);
  await page.click('button[type="submit"]');
  await page.waitForSelector('.app-header');
  await page.evaluate((dir) => { localStorage.setItem('nabiman_sftp_path_local', dir); localStorage.removeItem('nabiman_sftp_open'); }, DIR);
  await page.click('.cat-btn:has-text("원격")');
  await page.locator('.rw-item', { hasText: '로컬 셸' }).click();
  await page.click('.rw-tabs button:has-text("터미널")');
  const connect = await page.$('.rw-connect button:has-text("접속")');
  if (connect) await connect.click();
  const panel = page.locator('.rw-split-files');
  await panel.getByText('clip.webm', { exact: true }).waitFor({ timeout: 20000 });
  const row = (name) => panel.locator('.sftp-row', { hasText: name }).first();
  const dialog = page.locator('.sftp-preview');
  const facts = () => dialog.locator('.preview-facts').textContent();
  const close = async () => { await page.keyboard.press('Escape'); await dialog.waitFor({ state: 'detached' }); };
  const playing = (sel) => page.waitForFunction((s) => {
    const m = document.querySelector(`.sftp-preview ${s}`);
    return m && m.currentTime > 0.5 && !m.paused;
  }, sel, { timeout: 15000 }).then(() => true, () => false);

  // 1. A video plays straight away, a range at a time.
  await row('clip.webm').dblclick();
  await dialog.locator('video').waitFor({ timeout: 15000 });
  check('a video plays', await playing('video'));
  await page.waitForFunction(() => /640 × 360/.test(document.querySelector('.preview-facts')?.textContent || ''), null, { timeout: 10000 }).catch(() => {});
  check('its length and picture size are shown', /0:1[12]/.test(await facts()) && (await facts()).includes('640 × 360'), await facts());
  check('the player asks for ranges', streamed.some((r) => r.range.startsWith('bytes=')), JSON.stringify(streamed.slice(0, 2)));
  await page.evaluate(() => { document.querySelector('.sftp-preview video').currentTime = 9; });
  await page.waitForFunction(() => document.querySelector('.sftp-preview video')?.currentTime >= 9.3, null, { timeout: 10000 }).then(() => true, () => false)
    .then((ok) => check('seeking near the end plays on from there', ok));
  await page.screenshot({ path: `${OUT}/smoke-media-video.png` });
  await close();

  // 2. H.264 plays where the browser has the codec; otherwise it says so.
  await row('clip-h264.mp4').dblclick();
  await dialog.locator('video, .preview-note').first().waitFor({ timeout: 15000 });
  const h264 = await Promise.race([playing('video'), dialog.getByText(/재생할 수 없습니다/).waitFor({ timeout: 15000 }).then(() => 'unsupported', () => false)]);
  check('an MP4 plays, or says the browser cannot', h264 === true || h264 === 'unsupported', String(h264));
  await close();

  // 3. Songs, with ←/→ between them (the player keeps its own keys).
  for (const name of ['song.mp3', 'tone.ogg', 'tone.wav']) {
    await row(name).dblclick();
    await dialog.locator('audio').waitFor({ timeout: 15000 });
    check(`${name} plays`, await playing('audio'));
    if (name === 'song.mp3') await page.screenshot({ path: `${OUT}/smoke-media-audio.png` });
    await close();
  }

  // 4. An archive lists what it holds, Korean names too.
  await row('pack.zip').dblclick();
  await dialog.locator('.zip-preview').waitFor({ timeout: 15000 });
  const listing = await dialog.locator('.zip-preview').textContent();
  check('a ZIP lists its entries', listing.includes('readme.txt') && listing.includes('문서/보고서.txt') && listing.includes('docs/'), listing.slice(0, 120));
  check('with how many files and their size', (await facts()).includes('파일 2개'), await facts());
  await page.screenshot({ path: `${OUT}/smoke-media-zip.png` });
  await close();

  // 5. Anything else: what it is, by its first bytes.
  await row('tool').dblclick();
  await dialog.locator('.hex-preview').waitFor({ timeout: 15000 });
  check('a binary shows its kind and its bytes', (await facts()).includes('ELF') && (await dialog.locator('.hex-preview').textContent()).startsWith('00000000  7f 45 4c 46'), await facts());
  await page.screenshot({ path: `${OUT}/smoke-media-hex.png` });
  await close();
  await row('settings.myconf').dblclick();
  await dialog.locator('.text-preview').waitFor({ timeout: 15000 });
  check('a text with an unknown extension reads as text', (await dialog.locator('.text-preview').textContent()).includes('key = value'));
  await close();
  await row('empty.dat').dblclick();
  await dialog.getByText('빈 파일입니다.').waitFor({ timeout: 15000 }).catch(() => {});
  check('an empty file says so', await dialog.getByText('빈 파일입니다.').count() === 1);
  await close();

  check('nothing is fetched whole for a preview', !streamed.some((r) => !r.range && /tool|pack\.zip|settings/.test(r.url)), JSON.stringify(streamed.filter((r) => !r.range).map((r) => r.url.slice(-60))));
  check('no page errors and no browser dialogs', problems.length === 0, problems.join(' | '));
  await browser.close();
  const failed = results.filter((ok) => !ok).length;
  console.log(`${results.length - failed}/${results.length} checks passed`);
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.log('FATAL:', e.message.split('\n')[0]); process.exit(1); });
