#!/usr/bin/env node
/* CommissionApp — eski JavaScript ile yeni Rust kodunu karşılaştırır.
 *
 * Eski uygulamanın (git: REF, varsayılan f67f70a = son JS sürümü) fonksiyonlarını
 * çalıştırır ve aynı girdileri Rust'a (rust/commission-app/examples/parity.rs) verir:
 *   - parseTiffanyCode + inferCategory  (rastgele + bilinen ürün kodları, SKU'lar)
 *   - parseAmount, parseDate            (metin ve sayı biçimleri)
 *   - findHeaderRow, findPersonCol      (rastgele başlık tabloları)
 *   - parseHTMLTable                    (HTML tablo biçimleri)
 * Kullanım: node tools/check-parity.mjs [adet]   (varsayılan 200000 ürün kodu)
 * Çıkış kodu: 0 = hepsi aynı, 1 = fark var.
 */
import { execFileSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const REF = process.env.REF || 'f67f70a';
const N = Number(process.argv[2] || 200000);
const html = execFileSync('git', ['show', `${REF}:index.html`], { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 26 });

// ── Eski JS fonksiyonları ──
const grab = (start, endMarker) => { const a = html.indexOf(start); if (a < 0) throw new Error(start); const b = html.indexOf(endMarker, a); return html.slice(a, b); };
const src = [
  grab('const COLLECTION_CODES', '// ════════════════════════════════════════════════════════════════\n// COMMISSION CALCULATOR'),
  grab('function parseHTMLTable', '\nfunction findCol'),
  grab('function findCol', '\n// ── Auto-detect'),
  grab('function findHeaderRow', '\nfunction parseDate'),
  grab('function excelSerialToIso', '\n// v13.4'),
  grab('function parseDate', '\nasync function importExcelLocal'),
].join('\n');
const JS = new Function(src + '\nreturn { parseTiffanyCode, inferCategory, parseAmount, parseDate, findHeaderRow, findPersonCol, parseHTMLTable };')();

// ── Rastgele girdi üreticisi (tekrar üretilebilir) ──
let seed = 12345;
const rnd = () => ((seed = (seed * 1103515245 + 12345) >>> 0) / 4294967296);
const pick = a => a[Math.floor(rnd() * a.length)];
const tables = new Function(grab('const MATERIAL_CODES', '\n// ════') + grab('const SKU_TO_COLLECTION', '\nfunction lookupCollectionBySku') +
  '\nreturn { MATERIAL_CODES, CODE_FIXES, SKU_TO_COLLECTION };')();
const prio = /const PRIORITY_COLLECTIONS = (\{[\s\S]*?\});/.exec(html)[1];
const PRIO = new Function('return ' + prio)();
const VOCAB = [...new Set([
  ...Object.keys(PRIO), ...Object.keys(tables.MATERIAL_CODES), ...Object.keys(tables.CODE_FIXES), ...Object.values(tables.CODE_FIXES),
  'T', 'T1', 'T1N', 'T12', 'TW', 'TS', 'HW', 'HWIRE', 'HWLKBLT', 'HWDBL', 'LK', 'LKBLT', 'LOCKET', 'LOCKXXXXX', 'PKSA', 'SPLIT',
  'FNYKNOT', 'FLPVKNOT', 'RT2', 'RTT', 'RT', 'RETURNTOTIFFANY', '16STONE', '16XSTONE', 'SCHLUMB', 'FULLHRT', 'PUFFHT', 'TSMI',
  'PT18Y', 'PT18R', 'PT18W', '18YPT', '18RPT', 'PL18R', '95018Y', '18Y', '18R', '18W', 'SS', 'SSDI', 'SSHD', 'SSET', 'SST', 'SSG', 'SSER', 'SSIN',
  '925', 'AU', 'QZ', 'QTZ', 'SOL', 'STP36', 'WHT29', '29MM', '38', '40', '41', '1.02', '2.00', 'H', 'D', 'F', 'VS1', 'VVS2', 'IF', 'SI2', 'I1', 'FL',
  'WTCHMD', 'WATCH', 'WATCHES', 'ATLAS', 'ATL', 'USQ', 'OVCKTL', 'ETERN', 'RG', 'PDT', 'BLT', 'MD', 'SM', 'LRG', 'DI', 'RD', 'CU', 'EM', 'OV',
  'ÖZEL', 'ŞIK', 'ı', 'i', 'kn0t', 'hw', 'çanta', 'ß', ' ', 'X', '0', '7', '16IN', '18RHLFDIT1NRWHNGBGLMD', 'BONE', 'ELSA',
])];
const SEPS = [' ', ' ', ' ', '-', '/', '.', '  ', ' - ', ' ', '\t'];
const SKUS = Object.keys(tables.SKU_TO_COLLECTION);
const randCode = () => {
  const n = 1 + Math.floor(rnd() * 6);
  let s = '';
  for (let i = 0; i < n; i++) {
    let t = pick(VOCAB);
    if (rnd() < 0.15) t = t + pick(VOCAB);             // bitişik yazım
    if (rnd() < 0.1) t = t.toLowerCase();
    s += (i ? pick(SEPS) : (rnd() < 0.05 ? ' ' : '')) + t;
  }
  return s;
};
const randSku = () => { const r = rnd(); return r < 0.3 ? pick(SKUS) : r < 0.35 ? ' ' + pick(SKUS) + ' ' : r < 0.45 ? String(Math.floor(rnd() * 1e8)) : ''; };

const cases = [];
for (let i = 0; i < N; i++) cases.push({ k: 'code', code: randCode(), sku: randSku() });
const AMOUNTS = ['1.234,56', '1,234.56', '1,234', '12,5', '€ 31.000,00', '₺1.250', '-', '', 'abc', '12abc', '1e3', ' 99 ', '1.2.3', '0', '0,00', '-5', '1,2,3', '.5', '5.', 'Infinity'];
for (let i = 0; i < 3000; i++) cases.push({ k: 'amount', v: rnd() < 0.5 ? pick(AMOUNTS) : (rnd() < 0.5 ? Math.round(rnd() * 1e7) / 100 : `${Math.floor(rnd() * 99999)}${pick([',', '.'])}${Math.floor(rnd() * 999)}`) });
const DATES = ['31.08.2026', '1/2/26', '2026-8-1 10:00', '2026-08-31', '31-12-2026 23:59', '1.2.123', 'x', '', '45', 46310, 46310.75, 500, true];
for (let i = 0; i < 2000; i++) cases.push({ k: 'date', v: rnd() < 0.5 ? pick(DATES) : 30000 + Math.floor(rnd() * 30000) + (rnd() < 0.3 ? rnd() : 0) });
const HEADS = ['Belge Tarihi', 'Tarih', 'Date', 'Ürün Adı', 'Product', 'Malzeme No', 'SKU', 'Tutar', 'Amount', 'Fiyat', 'Liste Fiyat', 'Satışı Yapan',
  'Satıcı No', 'Seller ID', 'Kasiyer', 'Mağaza', 'Hasilat', 'Adet', 'Ad Soyad', '', 'Kod', 'Toplam', 'Açıklama', 0, 5];
for (let i = 0; i < 3000; i++) {
  const rows = [];
  const nr = 1 + Math.floor(rnd() * 8);
  for (let r = 0; r < nr; r++) { const w = Math.floor(rnd() * 7); rows.push(Array.from({ length: w }, () => pick(HEADS))); }
  cases.push({ k: 'header', rows });
}
const HT = ['<table><tr><th>A</th><td> b &amp; c </td></tr></table>', '﻿<html><TABLE border=1><TR><TD><span>31.08.2026</span></TD><TD>x&nbsp;y</TD></TR><tr></tr></TABLE>',
  '<table><thead><tr><th>Tarih</th></tr></thead><tbody><tr><td>&lt;1&gt; &quot;q&quot; &#39;s&#39;</td></tr></tbody></table><table><tr><td>2</td></tr></table>', 'no table', '<table><tr><td>a<td>b</td></tr></table>'];
for (const h of HT) cases.push({ k: 'html', html: h });

// ── Rust ──
const input = cases.map(c => JSON.stringify(c)).join('\n') + '\n';
const out = execFileSync('cargo', ['run', '-q', '--release', '-p', 'commission-app', '--example', 'parity'],
  { cwd: join(ROOT, 'rust'), input, maxBuffer: 1 << 28, encoding: 'utf8' }).trim().split('\n').map(l => JSON.parse(l));

// ── Karşılaştır ──
const fails = {};
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
cases.forEach((c, i) => {
  let js;
  if (c.k === 'code') { const p = JS.parseTiffanyCode(c.code, c.sku); js = [JS.inferCategory(p.collection, p.material), p.collection, p.material]; }
  else if (c.k === 'amount') js = JS.parseAmount(c.v);
  else if (c.k === 'date') js = JS.parseDate(c.v);
  else if (c.k === 'header') { const h = JS.findHeaderRow(c.rows); const heads = (c.rows[h] || []).map(x => String(x || '').trim().toLowerCase()); const p = JS.findPersonCol(heads); js = [h, p < 0 ? null : p]; }
  else if (c.k === 'html') js = JS.parseHTMLTable(c.html);
  if (!same(js, out[i])) (fails[c.k] = fails[c.k] || []).push({ input: c, js, rust: out[i] });
});
const kinds = [...new Set(cases.map(c => c.k))];
let bad = 0;
for (const k of kinds) {
  const n = cases.filter(c => c.k === k).length, f = (fails[k] || []).length;
  bad += f;
  console.log(`  ${f ? '✗' : '✓'} ${k.padEnd(7)} ${n.toLocaleString('de-DE')} girdi, ${f} fark`);
  (fails[k] || []).slice(0, 5).forEach(x => console.log('      ', JSON.stringify(x)));
}
console.log(bad ? `\nFARK VAR (${bad})` : '\nEski JS ile Rust AYNI ✓');
process.exit(bad ? 1 : 0);
