#!/usr/bin/env node
/* CommissionApp — uçtan uca karşılaştırma: eski JavaScript uygulaması ile yeni Rust uygulaması.
 *
 * Eski uygulamayı (git REF, varsayılan f67f70a = son JS sürümü v13.8) ve bu repodaki
 * uygulamayı gerçek tarayıcıda (Chromium) AYNI adımlarla çalıştırır ve karşılaştırır:
 *   elle satış girme, kayıtlı veri, Excel içe aktarma (xlsx + HTML ".xls"), içe aktarma
 *   tablosunda düzeltme, "Excel (My Sales)" EUR/TRY, bitmiş ay + hafta sonu kur günü,
 *   takım raporu (tek dosya EUR/TRY + kişi başına dosyalar), dosya adları, konsol hataları.
 * TCMB kuru ağdan alınmaz (sabit test kuru). "Generated On" saati karşılaştırılmaz.
 *
 * Gerekli: npm install --no-save playwright xlsx@0.18.5   (Chromium kurulu olmalı)
 *          rust/build-web.sh ile pkg/ derlenmiş olmalı
 * Kullanım: node tools/check-e2e.cjs        Çıkış kodu: 0 = tüm adımlar aynı
 */
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync, spawn } = require('child_process');
const { chromium } = require('playwright');
const XLSX = require('xlsx');
const ROOT = path.resolve(__dirname, '..');
const REF = process.env.REF || 'f67f70a';
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'commission-e2e-'));
fs.mkdirSync(TMP + '/old'); fs.mkdirSync(TMP + '/files');
fs.writeFileSync(TMP + '/old/index.html', execFileSync('git', ['show', `${REF}:index.html`], { cwd: ROOT, maxBuffer: 1 << 26 }));
const out = TMP + '/files/';

// ── Test dosyaları (SheetJS ile; eski uygulamanın da okuyabileceği biçimde) ──
let seed = 7; const rnd = () => ((seed = (seed * 1103515245 + 12345) >>> 0) / 4294967296); const pick = a => a[Math.floor(rnd() * a.length)];
const CODES = ['HW SM DBL LKBLT 18Y', 'SS RTT HRT TAG PDT', 'ST ATLAS AU STP TB 38', 'PT CU DI 1.02 H VS1', '18RHLFDIT1NRWHNGBGLMD',
  'NRW T WIRE BLT MD', 'PT18Y CIRCLE PDT 16IN', 'TIFFANY KNOT RG 18W', 'SS ELSA BONE CUFF', 'LOCK BANGLE 18R PAVE', 'RTT 2HRT PDT SS',
  '18Y T1 NRW RG', 'SS 1837 INLK DBL RG 6', 'WTCH EAST WEST', 'PT SOLITAIRE RG', 'SIXTEEN STONE RG 18Y', 'HARDWEAR LINK BLT SS',
  'TFKN0T DBL RG', 'SS KEYS PDT', 'PALOMA PICASSO X', 'ÖZEL SİPARİŞ ÜRÜN', 'USQ 36 QZ', 'SSDI PDT', '925 CHARM'];
const SKUS = ['60153062', '69525857', '60142140', '33263546', '', '', '', '12345678', '73580528'];
const PEOPLE = ['Ali Veli', 'Ali Veli', 'Ali Veli', 'AYŞE KAYA', 'ışık şen'];
const amount = () => { const v = Math.round(rnd() * 600000) / 100; const r = rnd();
  if (r < 0.35) return v; if (r < 0.5) return Math.round(rnd() * 3000) + 0.125 * Math.floor(rnd() * 8);
  if (r < 0.7) return v.toLocaleString('de-DE', { minimumFractionDigits: 2 }); if (r < 0.8) return '€ ' + v.toLocaleString('de-DE');
  if (r < 0.9) return v.toLocaleString('en-US', { minimumFractionDigits: 2 }); return String(Math.round(v)); };
const date = () => { const d = 1 + Math.floor(rnd() * 30), m = pick([9, 10, 10, 10]); const r = rnd();
  if (r < 0.4) return `${String(d).padStart(2, '0')}.${String(m).padStart(2, '0')}.2026`;
  if (r < 0.6) return 46266 + (m - 9) * 30 + d;  // Excel seri tarih
  if (r < 0.8) return `2026-${m}-${d}`; return `${d}/${m}/26`; };
const dataRows = (n, withPerson) => { const rows = [];
  for (let i = 0; i < n; i++) { const r = [date(), pick(SKUS), pick(CODES), amount()]; if (withPerson) r.push(pick(PEOPLE)); r.push('M01'); rows.push(r); }
  rows.splice(5, 0, [date(), '', '', 100, 'Ali Veli', 'M01']);         // boş kod
  rows.splice(9, 0, [date(), '1', 'HW LINK', 0, 'Ali Veli', 'M01']);     // sıfır tutar
  rows.splice(12, 0, ['yok', '1', 'HW LINK', 50, 'Ali Veli', 'M01']);    // bozuk tarih
  rows.splice(15, 0, []);                                                 // boş satır
  rows.push(['15.12.2099', '', 'SS RTT TAG', 75, 'Ali Veli', 'M01']);    // gelecek tarih
  return rows; };
// 1) ZeuPortal benzeri: özet blok + başlık 4. satırda
const zeu = [['Ad Soyad', 'Hasilat', 'Adet'], ['ALİ VELİ', 12345, 10], [],
  ['Belge Tarihi', 'Malzeme No', 'Ürün Adı', 'Tutar', 'Satışı Yapan', 'Mağaza Kodu'], ...dataRows(70, true)];
let wb = XLSX.utils.book_new(); XLSX.utils.book_append_sheet(wb, XLSX.utils.aoa_to_sheet(zeu), 'Rapor');
XLSX.writeFile(wb, out + 'zeu.xlsx');
// 2) HTML ".xls" (ZeuPortal)
const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;');
const htmlRows = zeu.map(r => '<tr>' + r.map(c => `<td>${esc(typeof c === 'number' && c > 40000 && c < 50000 ? '15.10.2026' : c)}</td>`).join('') + '</tr>').join('\n');
fs.writeFileSync(out + 'zeu-html.xls', '﻿<html><body><table border="1">\n' + htmlRows + '\n</table></body></html>');
// 3) Takım: kişi başına sayfa + tek sayfa "Satisi Yapan" + boş sayfa
wb = XLSX.utils.book_new();
const noPerson = [['Tarih', 'Malzeme No', 'Ürün', 'Tutar'], ...dataRows(25, false).map(r => r.length >= 5 ? [r[0], r[1], r[2], r[3]] : r)];
XLSX.utils.book_append_sheet(wb, XLSX.utils.aoa_to_sheet(noPerson), 'Mehmet Öz');
XLSX.utils.book_append_sheet(wb, XLSX.utils.aoa_to_sheet(zeu.slice(3)), 'Toplu');
XLSX.utils.book_append_sheet(wb, XLSX.utils.aoa_to_sheet([['x']]), 'Boş');
XLSX.writeFile(wb, out + 'team.xlsx');

const FILES = out;
const SHEETJS = require.resolve('xlsx/dist/xlsx.full.min.js');
const APPS = { old: { dir: TMP + '/old', port: 8702 }, new: { dir: ROOT, port: 8701 } };

// ── Karşılaştırma ──
const sleep = ms => new Promise(r => setTimeout(r, ms));

async function snapMain(p) {
  return p.evaluate(() => {
    const t = id => document.getElementById(id);
    return {
      badge: [t('targetBadge').textContent, t('targetBadge').style.background, t('targetBadge').style.color],
      progress: [t('progressText').textContent, t('progressFill').style.width, t('progressFill').className],
      total: t('totalPrim').textContent,
      breakdown: ['bSilver', 'bIcon', 'bOthers', 'bWatches'].map(i => t(i).textContent),
      footer: t('footerNote').textContent,
      fxNote: [t('fxNote').style.display, t('fxNote').innerHTML],
      blocks: [...document.querySelectorAll('.cat-block')].map(b => ({
        cls: b.className, rate: b.querySelector('.cat-title p').textContent,
        sum: b.querySelector('.sum').textContent, prim: b.querySelector('.cat-stats .prim').textContent,
        expanded: b.querySelector('.cat-head').getAttribute('aria-expanded'),
        empty: !!b.querySelector('.empty-hint'),
        rows: [...b.querySelectorAll('.row-item')].map(r => [r.dataset.rowid, r.title, r.querySelector('.num').textContent,
          r.querySelector('.date-input').value, r.querySelector('.day-name').textContent, r.querySelector('.date-wrap').title,
          r.querySelector('input[type=number]').value, r.querySelector('.row-prim').textContent]),
      })),
    };
  });
}
async function snapParse(p, statusId, boxId) {
  return p.evaluate(([s, b]) => ({
    status: [document.getElementById(s).className, document.getElementById(s).querySelector('div:last-child').textContent],
    pills: [...document.querySelectorAll(`#${b} .pill`)].map(x => x.textContent),
    rows: [...document.querySelectorAll(`#${b} table tbody tr`)].map(tr => [...tr.querySelectorAll('td')].map(td => {
      const i = td.querySelector('input,select'); return i ? i.value : td.textContent.trim() + '|' + (td.title || ''); })),
  }), [statusId, boxId]);
}
function readXlsx(buf) {
  const wb = XLSX.read(buf, { type: 'buffer' });
  return wb.SheetNames.map(n => {
    const aoa = XLSX.utils.sheet_to_json(wb.Sheets[n], { header: 1, defval: '', raw: true, blankrows: true });
    aoa.forEach(r => { if (r[0] === 'Generated On') r[1] = '<time>'; });
    while (aoa.length && aoa[aoa.length - 1].every(c => c === '')) aoa.pop();
    return { name: n, rows: aoa.map(r => { const c = r.slice(); while (c.length && c[c.length - 1] === '') c.pop(); return c; }) };
  });
}
async function download(p, click) {
  const [d] = await Promise.all([p.waitForEvent('download', { timeout: 30000 }), click()]);
  const path = await d.path();
  return { sheets: readXlsx(fs.readFileSync(path)) };
}

async function run(which) {
  const app = APPS[which];
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ acceptDownloads: true, locale: 'en-US', timezoneId: 'Europe/Istanbul' });
  const fxDates = [];
  await ctx.route('**/xlsx.full.min.js', r => r.fulfill({ path: SHEETJS, contentType: 'application/javascript' }));
  let fxDown = false;   // true: TCMB kaynaklarına hiç ulaşılamıyor (ağ hatası)
  for (const host of ['api.codetabs.com', 'api.allorigins.win', 'api.cors.lol', 'corsproxy.io', 'whateverorigin.org']) {
    await ctx.route(`https://${host}/**`, r => r.abort('failed'));
  }
  await ctx.route('https://tcmbproxy.sbakbulut.workers.dev/**', r => {
    if (fxDown) return r.abort('failed');
    const d = new URL(r.request().url()).searchParams.get('date'); fxDates.push(d);
    const day = new Date(d + 'T00:00:00Z').getUTCDay();
    return r.fulfill({ contentType: 'application/json', headers: { 'access-control-allow-origin': '*' },
      body: JSON.stringify(day === 0 || day === 6 ? { error: 'hafta sonu' } : { rate: 36.1234 + (d.endsWith('30') ? 0.5 : 0) }) });
  });
  await ctx.addInitScript(() => { window.__dl = []; const o = HTMLAnchorElement.prototype.click;
    HTMLAnchorElement.prototype.click = function () { if (this.download) window.__dl.push(this.download); return o.call(this); }; });
  const p = await ctx.newPage();
  const errors = [];
  p.on('pageerror', e => errors.push(e.message));
  p.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  p.on('dialog', d => d.accept());
  const R = {};
  await p.goto(`http://127.0.0.1:${app.port}/index.html`);
  await p.waitForFunction(() => document.querySelectorAll('.cat-block').length === 4, null, { timeout: 30000 });
  await sleep(300);
  R.a_fresh = await snapMain(p);

  // A) elle giriş
  await p.fill('#targetAmountInput', '1000');
  for (const [cat, vals] of Object.entries({ silver: ['117', '119'], icon: ['101', '1001.5'], others: ['31000', '250.5'], watches: ['4250.5'] })) {
    for (const v of vals) {
      await p.click(`.add-btn[data-add="${cat}"]`);
      const inputs = p.locator(`.rows[data-cat="${cat}"] .row-item input[type=number]`);
      await inputs.last().fill(v);
    }
  }
  R.a_entered = await snapMain(p);
  await p.locator('.rows[data-cat="icon"] .date-input').last().fill('2026-10-15');
  await p.locator('.rows[data-cat="icon"] .date-input').last().dispatchEvent('change');
  await p.locator('.rows[data-cat="silver"] .del').first().click();
  await p.click('.cat-head[data-toggle-cat="watches"]');
  await p.fill('#targetAmountInput', '100000');
  R.a_edited = await snapMain(p);
  await sleep(400);
  R.a_storage = await p.evaluate(() => { const d = JSON.parse(localStorage.getItem('prim_data_v2')); delete d.savedAt;
    return { d, s: localStorage.getItem('prim_settings_v1'), c: localStorage.getItem('prim_cat_collapsed') }; });
  R.a_export = await download(p, () => p.click('#exportExcelBtn'));

  // B) Excel içe aktarma (ZeuPortal xlsx)
  await p.click('#resetBtn'); await sleep(200);
  await p.click('#xlsxBtn');
  await p.setInputFiles('#xlsxFileInput', FILES + 'zeu.xlsx');
  await p.waitForFunction(() => !/okunuyor/.test(document.getElementById('xlsxStatusText').textContent), null, { timeout: 30000 });
  await sleep(200);
  R.b_parse = await snapParse(p, 'xlsxStatus', 'parseResults');
  await p.locator('#parseResults input[data-field="amount"]').nth(2).fill('999.99');
  await p.locator('#parseResults input[data-field="amount"]').nth(2).dispatchEvent('change');
  await p.locator('#parseResults select').nth(3).selectOption('watches');
  await p.locator('#parseResults [data-remove]').nth(1).click();
  R.b_parse_edit = await snapParse(p, 'xlsxStatus', 'parseResults');
  await p.click('#xlsxAddBtn'); await sleep(300);
  R.b_main = await snapMain(p);
  R.b_settings = await p.evaluate(() => localStorage.getItem('prim_settings_v1'));
  await p.fill('#targetAmountInput', '20000');
  R.b_export = await download(p, () => p.click('#exportExcelBtn'));
  await p.selectOption('#myCurrency', 'TRY');
  R.b_fxnote = await p.evaluate(() => document.getElementById('fxNote').innerHTML);
  R.b_export_try = await download(p, () => p.click('#exportExcelBtn'));
  R.b_fxnote2 = await p.evaluate(() => document.getElementById('fxNote').innerHTML);
  await p.selectOption('#myCurrency', 'EUR');

  // C) HTML ".xls"
  await p.click('#resetBtn'); await sleep(200);
  await p.click('#xlsxBtn');
  await p.setInputFiles('#xlsxFileInput', FILES + 'zeu-html.xls');
  await p.waitForFunction(() => !/okunuyor/.test(document.getElementById('xlsxStatusText').textContent), null, { timeout: 30000 });
  await sleep(200);
  R.c_parse = await snapParse(p, 'xlsxStatus', 'parseResults');
  await p.click('#xlsxAddBtn'); await sleep(300);
  R.c_main = await snapMain(p);

  // E) Bitmiş ay (Ekim 2026: 31'i Cumartesi → kur 30 Ekim Cuma) + TRY
  await p.click('#resetBtn'); await sleep(200);
  for (const [cat, v, d] of [['icon', '101', '2026-10-05'], ['silver', '117', '2026-10-31'], ['others', '31000', '2026-09-30']]) {
    await p.locator(`.rows[data-cat="${cat}"] input[type=number]`).first().fill(v);
    await p.locator(`.rows[data-cat="${cat}"] .date-input`).first().fill(d);
    await p.locator(`.rows[data-cat="${cat}"] .date-input`).first().dispatchEvent('change');
  }
  await p.selectOption('#myCurrency', 'TRY');
  R.e_export_try = await download(p, () => p.click('#exportExcelBtn'));
  R.e_fxnote = await p.evaluate(() => document.getElementById('fxNote').innerHTML);
  await p.selectOption('#myCurrency', 'EUR');

  // F) Ağ hatası: kur UYDURULMAZ, açık hata verilir (başka ay → önbellekteki kur kullanılmaz)
  await p.locator('.rows[data-cat="icon"] .date-input').first().fill('2026-08-14');
  await p.locator('.rows[data-cat="icon"] .date-input').first().dispatchEvent('change');
  await p.locator('.rows[data-cat="silver"] .date-input').first().fill('2026-08-14');
  await p.locator('.rows[data-cat="silver"] .date-input').first().dispatchEvent('change');
  await p.locator('.rows[data-cat="others"] .date-input').first().fill('2026-08-14');
  await p.locator('.rows[data-cat="others"] .date-input').first().dispatchEvent('change');
  fxDown = true;
  await p.selectOption('#myCurrency', 'TRY');
  await p.click('#exportExcelBtn');
  await p.waitForFunction(() => /failed/.test(document.querySelector('.export-toast')?.textContent || ''), null, { timeout: 60000 });
  R.f_offline = await p.evaluate(() => [document.querySelector('.export-toast').textContent, document.getElementById('exportExcelBtn').textContent,
    document.getElementById('exportExcelBtn').disabled, document.getElementById('fxNote').innerHTML]);
  fxDown = false;
  await p.selectOption('#myCurrency', 'EUR');

  // D) Takım raporu
  await p.click('#teamBtn');
  await p.fill('#teamTargetInput', '5000');
  await p.setInputFiles('#teamFileInput', FILES + 'team.xlsx');
  await p.waitForFunction(() => !/okunuyor/.test(document.getElementById('teamStatusText').textContent), null, { timeout: 30000 });
  await sleep(200);
  R.d_team = await snapParse(p, 'teamStatus', 'teamResults');
  await p.fill('#teamTargetInput', '20000');
  R.d_team2 = await snapParse(p, 'teamStatus', 'teamResults');
  R.d_export = await download(p, () => p.click('#teamExportBtn'));
  await p.selectOption('#teamCurrency', 'TRY');
  R.d_export_try = await download(p, () => p.click('#teamExportBtn'));
  R.d_status = await p.textContent('#teamStatusText');
  const seps = [];
  p.on('download', async d => { seps.push({ sheets: readXlsx(fs.readFileSync(await d.path())) }); });
  await p.click('#teamExportSepBtn');
  await p.waitForFunction(() => /separate files downloaded/.test(document.getElementById('teamStatusText').textContent), null, { timeout: 30000 });
  await sleep(1500);
  R.d_separate = seps.sort((a, b) => a.sheets[0].name.localeCompare(b.sheets[0].name));
  R.fxDates = fxDates;
  R.fileNames = await p.evaluate(() => window.__dl.map(n => n.replace(/\d{4}-\d{2}-\d{2}/, '<today>')));
  R.errors = errors;
  await browser.close();
  return R;
}

(async () => {
  const servers = Object.values(APPS).map(a => spawn('python3', ['-m', 'http.server', String(a.port), '--bind', '127.0.0.1'], { cwd: a.dir, stdio: 'ignore' }));
  await sleep(800);
  try {
    const [o, n] = await Promise.all([run('old'), run('new')]);
    fs.writeFileSync(TMP + '/result-old.json', JSON.stringify(o, null, 1));
    fs.writeFileSync(TMP + '/result-new.json', JSON.stringify(n, null, 1));
    let diffs = 0;
    for (const k of Object.keys(o)) {
      const a = JSON.stringify(o[k]), b = JSON.stringify(n[k]);
      if (a === b) { console.log('  ✓', k); continue; }
      diffs++; console.log('  ✗', k);
      // ilk farkı göster
      const flat = (x, pre = '') => typeof x !== 'object' || x === null ? { [pre]: x } : Object.assign({}, ...Object.entries(x).map(([kk, v]) => flat(v, pre + '/' + kk)));
      const fa = flat(o[k]), fb = flat(n[k]);
      const keys = [...new Set([...Object.keys(fa), ...Object.keys(fb)])].filter(x => JSON.stringify(fa[x]) !== JSON.stringify(fb[x]));
      keys.slice(0, 6).forEach(x => console.log(`      ${x}\n        old: ${JSON.stringify(fa[x])}\n        new: ${JSON.stringify(fb[x])}`));
      if (keys.length > 6) console.log(`      … ${keys.length} fark`);
    }
    console.log(diffs ? `\n${diffs} adımda fark var (ayrıntı: ${TMP})` : '\nTÜM ADIMLAR AYNI ✓');
    process.exitCode = diffs ? 1 : 0;
  } finally { servers.forEach(s => s.kill()); }
})().catch(e => { console.error(e); process.exit(1); });
