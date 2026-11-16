#!/usr/bin/env node
/* CommissionApp — Rust/WASM hesap çekirdeği denetleyicisi (bağımlılık yok).
 *
 * index.html içindeki hesap çekirdeğini (CommissionCore) Node.js'te çalıştırır ve:
 *   1) Gömülü WASM yükleniyor mu, motor 'rust-wasm' mı
 *   2) Bilinen örnekler doğru mu (101 € × %1,5 = 1,52 € vb.)
 *   3) WASM ile JS yedeği, 0,01–60.000 € arası HER cent için aynı sonucu veriyor mu
 *      (4 kategori × hedef tuttu/tutmadı) — kurallar iki yerde ayrışırsa yakalar
 *   4) WASM ile bağımsız BigInt referansı aynı mı (oran tablosu index.html'den)
 *   5) TL çevirisi (kur 4 ondalık) WASM ve JS yedeğinde aynı mı
 *   6) (isteğe bağlı) Gömülü WASM, rust/ altındaki güncel derleme ile aynı mı
 *
 * Kullanım: node tools/check-wasm.mjs
 * Çıkış kodu: 0 = tüm kontroller geçti, 1 = hata var.
 */
import { readFileSync, existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const html = readFileSync(join(ROOT, 'index.html'), 'utf8');

let passed = 0;
const failures = [];
function check(label, condition, detail = '') {
  if (condition) { passed++; console.log(`  ✓ ${label}`); }
  else { failures.push(label); console.log(`  ✗ ${label}${detail ? ' — ' + detail : ''}`); }
}

// ── index.html'den gerekli kodu çıkar ────────────────────────────────
const between = (start, end) => {
  const a = html.indexOf(start), b = html.indexOf(end, a);
  if (a < 0 || b < 0) throw new Error(`index.html içinde bulunamadı: ${start}`);
  return html.slice(a, b + end.length);
};
const rulesSrc = between('const baseRates', 'const highValueRate = 0.02;');
const coreSrc = between('const COMMISSION_WASM_B64', '})();');
const catsSrc = between('const CATS = [', '];');

// Sayfadaki sıra ile aynı: kurallar → çekirdek → CATS (CATS çekirdekten SONRA tanımlıdır).
const load = (rules) => {
  const warnings = [];
  const ctx = vm.createContext({ atob, WebAssembly, console: { info() {}, warn: (...a) => warnings.push(a.join(' ')) } });
  vm.runInContext(`${rules}\n${coreSrc}\n${catsSrc}\nthis.CommissionCore = CommissionCore; this.R = { baseRates, targetRates, highValueThreshold, highValueRate, CATS };`, ctx);
  return { core: ctx.CommissionCore, R: ctx.R, warnings };
};
const { core, R, warnings } = load(rulesSrc);

console.log('\nRust/WASM hesap çekirdeği');
check('gömülü WASM yüklendi (engine = rust-wasm)', core.engine === 'rust-wasm', `engine = ${core.engine}; ${warnings.join(' ')}`);
const changed = load(rulesSrc.replace(/icon: 0\.015/, 'icon: 0.02'));
check('JS oranı değişirse WASM devre dışı kalır, JS yedeği yeni oranı kullanır',
  changed.core.engine === 'js' && changed.core.commission('icon', 101, false) === 2.02,
  `engine = ${changed.core.engine}`);

// WASM dışa aktarımlarına doğrudan eriş (JS yedeği ile karşılaştırmak için)
const b64 = /const COMMISSION_WASM_B64 = '([^']*)'/.exec(html)[1];
const wasm = new WebAssembly.Instance(new WebAssembly.Module(Buffer.from(b64, 'base64')), {}).exports;
const js = core.jsCore;

console.log('\nBilinen örnekler');
const examples = [
  ['icon', 101, false, 1.52], ['icon', 105, false, 1.58], ['silver', 117, false, 0.59],
  ['silver', 119, false, 0.60], ['icon', 101, true, 3.03], ['others', 31000, true, 620],
  ['others', 31000, false, 310], ['others', 30999.99, true, 310], ['watches', 4250.5, true, 127.52],
];
for (const [cat, v, met, want] of examples) {
  const got = core.commission(cat, v, met);
  check(`${cat} ${v} € (hedef ${met ? 'tuttu' : 'yok'}) → ${want} €`, got === want, `sonuç ${got}`);
}
check('10 × 0,10 € toplamı = 1 € ve hedef 1 € tutar',
  core.sum(Array(10).fill(0.1)) === 1 && core.targetMet(core.sum(Array(10).fill(0.1)), 1));
check('hedef 0 → "ayarlanmadı" (tutmadı sayılır)', core.targetMet(100, 0) === false);
check('1,52 € × 36,1234 = 54,91 ₺', core.convert(1.52, 36.1234) === 54.91, `sonuç ${core.convert(1.52, 36.1234)}`);
let threw = false; try { core.commission('gold', 1, false); } catch { threw = true; }
check('bilinmeyen kategori hata verir', threw);

// ── Kapsamlı karşılaştırma ───────────────────────────────────────────
console.log('\nKapsamlı karşılaştırma (her cent, 0,01–60.000 €)');
const MAX = 6_000_000n;
const bp = r => BigInt(Math.round(r * 10000));
const ref = (cat, amount, met) => {                       // bağımsız referans
  const c = R.CATS[cat];
  const rate = c === 'others' && met && amount >= BigInt(R.highValueThreshold * 100)
    ? bp(R.highValueRate) : bp((met ? R.targetRates : R.baseRates)[c]);
  const p = amount * rate;
  return p / 10000n + ((p % 10000n) * 2n >= 10000n ? 1n : 0n);
};
let wasmVsJs = 0, wasmVsRef = 0, firstDiff = '';
for (let cat = 0; cat < 4; cat++) {
  for (const met of [0, 1]) {
    for (let a = 1n; a <= MAX; a++) {
      const w = wasm.commission_cents(cat, a, met);
      if (w !== js.commission_cents(cat, a, met)) { wasmVsJs++; firstDiff ||= `cat ${cat} ${a} cent`; }
      if (w !== ref(cat, a, met)) { wasmVsRef++; firstDiff ||= `cat ${cat} ${a} cent (ref)`; }
    }
  }
}
const n = (4 * 2 * Number(MAX)).toLocaleString('de-DE');
check(`WASM = JS yedeği (${n} hesap)`, wasmVsJs === 0, `${wasmVsJs} fark, ilk: ${firstDiff}`);
check(`WASM = bağımsız referans (${n} hesap)`, wasmVsRef === 0, `${wasmVsRef} fark, ilk: ${firstDiff}`);

let fxDiff = 0;
for (const fx of [10001n, 361234n, 352999n, 500000n]) {
  for (let a = 1n; a <= 200_000n; a++) if (wasm.convert_cents(a, fx) !== js.convert_cents(a, fx)) fxDiff++;
}
check('TL çevirisi: WASM = JS yedeği (800.000 hesap)', fxDiff === 0, `${fxDiff} fark`);

// ── Gömülü WASM güncel mi? ───────────────────────────────────────────
const built = join(ROOT, 'rust/target/wasm32-unknown-unknown/release/commission_wasm.wasm');
if (existsSync(built)) {
  console.log('\nDerleme');
  check('gömülü WASM = rust/target derlemesi (rust/build-wasm.sh çalıştırıldı)',
    Buffer.from(b64, 'base64').equals(readFileSync(built)));
}

console.log(`\n${passed} geçti, ${failures.length} başarısız`);
process.exit(failures.length ? 1 : 0);
