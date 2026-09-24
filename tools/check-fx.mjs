#!/usr/bin/env node
/* CommissionApp — TRY kur çevrimi denetleyicisi (bağımlılık yok, ağ erişimi yok).
 *
 * v13.8 kuralını kanıtlar: raporda geçen EN SON satış ayının (rapor ayı) son iş günü
 * TCMB döviz satış kuru, raporun TÜM satırlarına TEK kur olarak uygulanır
 * (satır başına günlük kur ya da ay başına ayrı kur yoktur).
 *
 * Kontroller:
 *   1) index.html'deki kur kodu tek kur modelini kullanıyor mu (fxFor tarihi yok sayar,
 *      ay bazlı kur kalıntısı yok, yükleyici tek hedef gün çeker)
 *   2) rapor ayı ve hedef gün hesabı (ay bittiyse ayın son günü, sürüyorsa bugün)
 *   3) kur yüklendikten sonra farklı ay/gündeki satırlar AYNI kuru mu kullanıyor
 *   4) Excel'e yazılan "FX Source" ve arayüz özeti tek kuru mu bildiriyor
 *   5) ağ hatasında kur uydurulmuyor ve aynı hedef gün için cache çalışıyor
 *   6) <title> / sw.js sürümü ve gömülü JS sözdizimi
 *
 * Kullanım: node tools/check-fx.mjs
 * Çıkış kodu: 0 = tüm kontroller geçti, 1 = hata var.
 */
import { readFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const read = (rel) => readFileSync(join(ROOT, rel), 'utf8');

let passed = 0;
const failures = [];

function check(label, condition, detail = '') {
  if (condition) {
    passed++;
    console.log(`  OK    ${label}`);
  } else {
    failures.push(label + (detail ? ` — ${detail}` : ''));
    console.log(`  HATA  ${label}${detail ? ` — ${detail}` : ''}`);
  }
}

const html = read('index.html');
const mainStart = html.indexOf('<script>');
const mainEnd = html.indexOf('</script>', mainStart);
const script = html.slice(mainStart + '<script>'.length, mainEnd);

/* Bir fonksiyonu (async dahil) süslü parantez dengesiyle kaynaktan keser. */
function extractFunction(src, signature) {
  const start = src.indexOf(signature);
  if (start < 0) return '';
  let i = src.indexOf('{', start);
  let depth = 0;
  for (; i < src.length; i++) {
    if (src[i] === '{') depth++;
    else if (src[i] === '}') { depth--; if (depth === 0) break; }
  }
  return src.slice(start, i + 1);
}

/* Uygulamanın gerçek FX bölümü: '// ── FX (EUR → TRY, TCMB' ... 'function tcmbXmlUrl(' */
function fxSection(src) {
  const start = src.indexOf('// ── FX (EUR → TRY, TCMB');
  const end = src.indexOf('function tcmbXmlUrl(', start);
  if (start < 0 || end < 0) return '';
  return src.slice(start, end);
}

console.log('CommissionApp — TRY kur çevrimi denetimi (v13.8: rapor ayı son iş günü, tek kur)\n');

/* ── 1) statik kaynak kontrolleri ───────────────────────────────────────────── */
console.log('index.html — kur kodu (statik)');
check('FX bölümü bulundu', fxSection(script).length > 0);
check('fxFor tarihi yok sayıyor (tüm satırlar tek kuru kullanır)',
  /function fxFor\(_date\)\s*\{\s*return fxRate \? fxRate\.rate : 0;\s*\}/.test(script));
check('ay bazlı kur kalıntısı yok (fxRates)',
  !/fxRates/.test(script) && !/fxRates/.test(html));
check('rapor ayı fonksiyonu var (fxReportMonth)', /function fxReportMonth\(isoDates\)/.test(script));
check('rapor ayı = dosyadaki EN SON satış ayı',
  /return months\.length \? months\[months\.length - 1\] : monthOf\(todayISO\(\)\);/.test(script));
check('hedef gün hâlâ ayın SON takvim günü (süren ayda bugün)',
  /const monthEnd = new Date\(Date\.UTC\(y, m, 0\)\);/.test(script)
  && /return \{ ds: \(isCurrent \? todayUTC : monthEnd\)\.toISOString\(\)\.slice\(0, 10\), isCurrent \};/.test(script));
check('yükleyici tek hedef gün çekiyor (ay döngüsü yok)',
  /const month = fxReportMonth\(isoDates\);/.test(script)
  && /const \{ ds, isCurrent \} = fxTargetForMonth\(month\);/.test(script)
  && !/for \(const mo of months\)/.test(script));
check('kur hedefe göre cache\'leniyor (aynı hedef gün → tek istek)',
  /if \(fxRate && fxRate\.target === ds && fxRate\.rate > 0\) return;\s*\/\/ cache/.test(script));
check('Excel "FX Source" tek kur + rapor ayı bildiriyor',
  /son iş günü TEK kuru/.test(script) && /rapor ayı \(\$\{fxRate\.month\}\)/.test(script));
check('arayüz notu tek kuru bildiriyor', /tüm satırlara tek kur/.test(script));
check('para birimi etiketi iki seçicide de "rapor ayı son kuru"',
  (html.match(/₺ TRY \(rapor ayı son kuru\)/g) || []).length === 2);

/* ── 2) sürüm tutarlılığı + sözdizimi ──────────────────────────────────────── */
console.log('\nsürüm + sözdizimi');
const titleVer = (html.match(/<title>[^<]*?(v\d+\.\d+)<\/title>/) || [])[1];
const swVer = (read('sw.js').match(/const VERSION = '(v[\d.]+)'/) || [])[1];
check('sw.js sürümü <title> ile aynı', Boolean(titleVer) && titleVer === swVer, `${titleVer} / ${swVer}`);
check('README tek kur davranışını belgeliyor',
  /rapor ayı son iş günü/.test(read('README.md')) && /rapor genelinde \*\*tek\*\* kur/.test(read('README.md')));

const tmpDir = mkdtempSync(join(tmpdir(), 'commission-fx-'));
const jsBlocks = [['index.html gömülü JS', script]];
const pwaStart = html.indexOf('<script>', mainEnd);
if (pwaStart >= 0) {
  jsBlocks.push(['index.html PWA bloğu', html.slice(pwaStart + '<script>'.length, html.indexOf('</script>', pwaStart))]);
}
for (const [label, code] of jsBlocks) {
  const file = join(tmpDir, label.replace(/\W+/g, '_') + '.js');
  writeFileSync(file, code);
  try {
    execFileSync(process.execPath, ['--check', file], { stdio: 'pipe' });
    check(`sözdizimi geçerli: ${label}`, true);
  } catch (err) {
    check(`sözdizimi geçerli: ${label}`, false, String(err.stderr || err.message).split('\n')[0]);
  }
}

/* ── 3) davranış: uygulamanın GERÇEK FX kodu sahte ağ ile çalıştırılır ─────── */
/* Kur kodu index.html'den kesilir; DOM ve ağ yerine stub'lar verilir; TCMB
 * cevabı taklit edilir (56.0853 = 31.08.2026 döviz satış kuru). */
const PRELUDE = `
const __state = { currency: 'TRY', fetchLog: [], fetchFails: false, rate: 56.0853, rateDate: '2026-08-31' };
function $(id) { return { value: __state.currency, style: {}, innerHTML: '' }; }
function renderFxNote() {}                       // DOM stub
async function fetchRateOnOrBefore(ds) {         // ağ stub
  __state.fetchLog.push(ds);
  if (__state.fetchFails) throw new Error('TCMB kuru alınamadı — internet bağlantınızı kontrol edin. (simüle)');
  return { rate: __state.rate, date: __state.rateDate };
}
`;

const FX_BLOCK = fxSection(script);
const TODAY_ISO = extractFunction(script, 'function todayISO()');
const LOADER = extractFunction(script, 'async function loadFxRatesForDates(');

check('sandbox için FX bölümü/loader/todayISO kaynaktan çıkarıldı',
  FX_BLOCK.length > 0 && LOADER.length > 0 && TODAY_ISO.length > 0);

const TESTS = `
(async () => {
  try {
    const today = todayISO();
    const thisMonth = today.slice(0, 7);
    const ym = (offset) => { const d = new Date(Date.UTC(+today.slice(0, 4), +today.slice(5, 7) - 1 + offset, 1)); return d.toISOString().slice(0, 7); };
    const monthEnd = (m) => { const p = m.split('-').map(Number); return new Date(Date.UTC(p[0], p[1], 0)).toISOString().slice(0, 10); };
    const M1 = ym(-1), M2 = ym(-2), M3 = ym(-3);      // bitmiş aylar
    const D1 = M1 + '-10', D2 = M2 + '-05';
    const mixed = [D2, M2 + '-28', D1, M1 + '-22'];

    check('rapor ayı = dosyadaki en son satış ayı', fxReportMonth(mixed) === M1, fxReportMonth(mixed));
    check('rapor ayı: satır yoksa bugünün ayı', fxReportMonth([]) === thisMonth, fxReportMonth([]));
    check('rapor ayı: boş/eksik tarihler yok sayılıyor', fxReportMonth(['', null, D2]) === M2, fxReportMonth(['', null, D2]));

    const past = fxTargetForMonth(M1);
    check('geçmiş ay hedefi = ayın son takvim günü', past.ds === monthEnd(M1) && past.isCurrent === false, JSON.stringify(past));
    check('süren ay hedefi = bugün', fxTargetForMonth(thisMonth).ds === today);
    const future = fxTargetForMonth(ym(+3));
    check('gelecek ay hedefi = bugün + isCurrent', future.ds === today && future.isCurrent === true, JSON.stringify(future));
    check('lastDayOfReportMonth = rapor ayının son günü', lastDayOfReportMonth(mixed) === monthEnd(M1), lastDayOfReportMonth(mixed));

    check('kur yüklenmeden çevrim 0 (kur uydurulmaz)', fxFor(D1) === 0 && fxFor(D2) === 0);

    __state.rate = 56.0853;
    __state.rateDate = monthEnd(M1);
    await loadFxRatesForDates(mixed);
    check('ağdan istenen gün = rapor ayının son günü (tek istek)',
      __state.fetchLog.length === 1 && __state.fetchLog[0] === monthEnd(M1), __state.fetchLog.join(','));
    check('tek kur yüklendi (56.0853 / rapor ayı son günü)',
      fxRate && fxRate.rate === 56.0853 && fxRate.date === monthEnd(M1) && fxRate.month === M1 && fxRate.isCurrent === false,
      JSON.stringify(fxRate));
    check('FARKLI AYLARDAKİ satırlar aynı kuru kullanıyor',
      fxFor(D2) === 56.0853 && fxFor(D1) === 56.0853 && fxFor(M3 + '-15') === 56.0853 && fxFor(today) === 56.0853);
    check('TL hesabı tek kurla: 10.000 EUR → 560.853 TL, 100 EUR prim → 5.608,53 TL',
      n2(10000 * fxFor(D2)) === 560853 && n2(100 * fxFor(D1)) === 5608.53);

    await loadFxRatesForDates(mixed);
    check('aynı hedef gün için ikinci çağrı ağ isteği yapmaz (cache)', __state.fetchLog.length === 1);

    __state.fetchLog.length = 0;
    __state.rate = 57.5;
    __state.rateDate = monthEnd(M2);
    await loadFxRatesForDates([D2]);
    check('rapor ayı değişince yeni tek kur çekilir',
      __state.fetchLog.length === 1 && __state.fetchLog[0] === monthEnd(M2), __state.fetchLog.join(','));
    check('yeni kur tüm satırlara (eski/yeni ay dahil) uygulanır',
      fxFor(D1) === 57.5 && fxFor(M3 + '-15') === 57.5);
    const expectedLabel = 'T.C. Merkez Bankası (TCMB) döviz satış kuru — rapor ayı (' + M2 + ') son iş günü TEK kuru: ' + monthEnd(M2) + ' = 57.5 — tüm satırlara aynı kur uygulandı';
    check('FX Source etiketi rapor ayı + tek kuru yazar', fxSourceLabel() === expectedLabel, fxSourceLabel());
    check('kısa özet tek kur', fxRateSummaryShort() === monthEnd(M2) + ' (57.5)', fxRateSummaryShort());

    await loadFxRatesForDates([today]);
    if (today === monthEnd(thisMonth)) {
      check('ayın son günü: isCurrent false (ay bitti)', fxRate.isCurrent === false, JSON.stringify(fxRate));
    } else {
      check('süren ay için [ay sürüyor] etiketi', /\\[ay sürüyor\\]/.test(fxSourceLabel()), fxSourceLabel());
    }

    const before = fxRate;
    __state.fetchFails = true;
    let err = null;
    try { await loadFxRatesForDates([M3 + '-15']); } catch (e) { err = e; }
    check('ağ hatasında hata verilir ve kur değişmez (kur uydurulmaz)', Boolean(err) && fxRate === before, err && err.message);
    __state.fetchFails = false;

    __state.fetchLog.length = 0;
    __state.rate = 58.25;
    __state.rateDate = monthEnd(M3);
    await loadFxRatesForDates([M3 + '-15']);
    check('hata sonrası yeni tek kur çekilir ve tüm satırlara uygulanır',
      fxFor(D1) === 58.25 && __state.fetchLog[0] === monthEnd(M3), __state.fetchLog.join(','));
  } catch (err) {
    check('davranış testleri çalıştı', false, err && err.stack ? err.stack.split('\\n')[0] : String(err));
  } finally {
    __finish();
  }
})();
`;

const sandbox = { check, console };
let finish;
const done = new Promise((resolvePromise) => { finish = resolvePromise; });
sandbox.__finish = finish;

console.log('\ndavranış (index.html kur kodu, sahte ağ)');
vm.createContext(sandbox);
vm.runInContext([PRELUDE, FX_BLOCK, TODAY_ISO, LOADER, TESTS].join('\n'), sandbox, { filename: 'commission-fx-sandbox.js' });
await done;

console.log(`\n${passed} kontrol geçti, ${failures.length} hata.`);
if (failures.length) {
  console.log('\nHATALAR:');
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log('SONUÇ: TRY çevrimi rapor ayının son iş günü kuruyla TEK kur olarak yapılıyor.');


