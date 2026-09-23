#!/usr/bin/env node
/* CommissionApp — PWA kurulabilirlik denetleyicisi (bağımlılık yok).
 *
 * Chrome'un "Uygulamayı yükle" (install) ölçütlerini ve repodaki dosyaların
 * tutarlılığını denetler:
 *   1) manifest.json geçerli JSON ve zorunlu alanları içeriyor mu
 *   2) 192x192 + 512x512 ikonlar (purpose: any) manifest'te var mı ve
 *      diskte gerçekten o ölçüde PNG olarak duruyor mu
 *   3) index.html manifest'i, tema rengini, apple meta etiketlerini ve
 *      service worker kaydını içeriyor mu
 *   4) index.html'e gömülü PWA blokları pwa/*.html ile birebir aynı mı
 *      (publish.sh enjeksiyonunun bozulmadığını kanıtlar)
 *   5) sw.js sözdizimi geçerli mi ve önbelleğe aldığı kabuk dosyaları var mı
 *
 * Kullanım: node tools/check-pwa.mjs
 * Çıkış kodu: 0 = tüm kontroller geçti, 1 = hata var.
 */
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

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

function pngInfo(rel) {
  const buf = readFileSync(join(ROOT, rel));
  const signature = buf.subarray(0, 8).toString('latin1') === '\x89PNG\r\n\x1a\n';
  // IHDR her zaman ilk chunk'tır: 8 imza + 4 uzunluk + 4 tip + 8 (w,h)
  const width = buf.readUInt32BE(16);
  const height = buf.readUInt32BE(20);
  const depth = buf[24];
  const colorType = buf[25];
  const hasIend = buf.subarray(buf.length - 8).toString('latin1').includes('IEND');
  return { signature, width, height, depth, colorType, hasIend, bytes: buf.length };
}

console.log('CommissionApp — PWA kurulabilirlik denetimi\n');

/* ── 1) manifest.json ─────────────────────────────────────────────────────── */
console.log('manifest.json');
let manifest = null;
try {
  manifest = JSON.parse(read('manifest.json'));
} catch (err) {
  check('geçerli JSON', false, err.message);
}
if (manifest) {
  check('geçerli JSON', true);
  check('name veya short_name var', Boolean(manifest.name || manifest.short_name));
  check('start_url var', typeof manifest.start_url === 'string' && manifest.start_url.length > 0,
    JSON.stringify(manifest.start_url));
  check('scope var', typeof manifest.scope === 'string');
  const displays = ['fullscreen', 'standalone', 'minimal-ui', 'window-controls-overlay'];
  check('display uygun', displays.includes(manifest.display), String(manifest.display));
  check('prefer_related_applications yok/false',
    manifest.prefer_related_applications === undefined || manifest.prefer_related_applications === false);
  check('theme_color tanımlı', typeof manifest.theme_color === 'string', String(manifest.theme_color));
  check('icons dizisi var', Array.isArray(manifest.icons) && manifest.icons.length > 0);

  const icons = Array.isArray(manifest.icons) ? manifest.icons : [];
  for (const size of ['192x192', '512x512']) {
    const anys = icons.filter((i) => (i.purpose === undefined || i.purpose === 'any') &&
      String(i.sizes).split(/\s+/).includes(size));
    check(`${size} "any" ikonu manifest'te`, anys.length > 0);
    for (const icon of anys) {
      if (!existsSync(join(ROOT, icon.src))) {
        check(`ikon dosyası var: ${icon.src}`, false, 'dosya yok');
        continue;
      }
      const info = pngInfo(icon.src);
      check(`ikon dosyası var: ${icon.src}`, true);
      check(`  ${icon.src} PNG imzası`, info.signature);
      check(`  ${icon.src} ölçü ${size}`,
        `${info.width}x${info.height}` === size, `${info.width}x${info.height}`);
      check(`  ${icon.src} 8-bit RGBA`, info.depth === 8 && info.colorType === 6,
        `depth=${info.depth} colorType=${info.colorType}`);
      check(`  ${icon.src} IEND var`, info.hasIend);
    }
  }
  const maskable = icons.filter((i) => i.purpose === 'maskable');
  check('en az bir maskable ikon (Android uyarlanır simge)', maskable.length > 0);
  for (const icon of maskable) check(`maskable dosyası var: ${icon.src}`, existsSync(join(ROOT, icon.src)));
  for (const icon of icons) check(`tüm manifest ikon dosyaları var: ${icon.src}`, existsSync(join(ROOT, icon.src)));
}

/* ── 2) index.html <head> ve kayıt kodu ───────────────────────────────────── */
console.log('\nindex.html');
const html = read('index.html');
check('manifest bağlantısı', /<link\s+rel="manifest"\s+href="\.\/manifest\.json"/.test(html));
check('theme-color meta', /<meta\s+name="theme-color"/.test(html));
check('apple-touch-icon', /<link\s+rel="apple-touch-icon"/.test(html));
check('mobile-web-app-capable', /<meta\s+name="mobile-web-app-capable"\s+content="yes"/.test(html));
check('apple-mobile-web-app-capable', /<meta\s+name="apple-mobile-web-app-capable"/.test(html));
check('service worker kaydı (./sw.js)', /serviceWorker\.register\(\s*'\.\/sw\.js'/.test(html));
check('beforeinstallprompt yakalanıyor', html.includes("addEventListener('beforeinstallprompt'"));
check('uygulama adı <title> sürüm içeriyor', /<title>[^<]*v\d+\.\d+/.test(html));
check('<meta charset> ilk sıralarda', html.slice(0, 400).includes('<meta charset="UTF-8"'));

/* ── 3) gömülü bloklar pwa/*.html ile aynı mı ─────────────────────────────── */
console.log('\npwa blokları (publish.sh enjeksiyonu)');
function block(raw, marker) {
  const start = raw.indexOf(`<!-- ${marker}:START`);
  const endMarker = `<!-- ${marker}:END -->`;
  const end = raw.indexOf(endMarker);
  if (start < 0 || end < 0) return null;
  return raw.slice(start, end + endMarker.length);
}
for (const [marker, file] of [['PWA:HEAD', 'pwa/head.html'], ['PWA:BODY', 'pwa/body-end.html']]) {
  const inFile = block(read(file), marker);
  const inHtml = block(html, marker);
  check(`blok dosyası geçerli: ${file}`, Boolean(inFile));
  check(`index.html içindeki ${marker} bloğu ${file} ile birebir aynı`,
    Boolean(inHtml) && inHtml === inFile);
}

/* ── 4) service worker ────────────────────────────────────────────────────── */
console.log('\nsw.js');
check('sw.js var', existsSync(join(ROOT, 'sw.js')));
if (existsSync(join(ROOT, 'sw.js'))) {
  const sw = read('sw.js');
  check('fetch olayı dinleniyor', /addEventListener\(\s*'fetch'/.test(sw));
  check('navigasyon istekleri network-first', /networkFirst/.test(sw) && /mode === 'navigate'/.test(sw));
  check('sürüm sabiti var', /const VERSION = 'v\d/.test(sw));
  try {
    execFileSync(process.execPath, ['--check', join(ROOT, 'sw.js')], { stdio: 'pipe' });
    check('sözdizimi geçerli (node --check)', true);
  } catch (err) {
    check('sözdizimi geçerli (node --check)', false, String(err.stderr || err.message).split('\n')[0]);
  }
  const assets = [...sw.matchAll(/'\.\/([^']*)'/g)].map((m) => m[1])
    .filter((p) => p === '' || /\.[a-z]+$/.test(p));
  for (const asset of new Set(assets)) {
    const rel = asset === '' ? 'index.html' : asset;
    check(`önbellek kabuğunda: ${rel}`, existsSync(join(ROOT, rel)));
  }
}

/* ── 5) repo genel bakış ──────────────────────────────────────────────────── */
console.log('\nrepo');
const iconsDir = join(ROOT, 'icons');
check('icons/ klasörü var', existsSync(iconsDir));
if (existsSync(iconsDir)) console.log(`  BILGI icons/: ${readdirSync(iconsDir).join(', ')}`);
check('favicon.svg var', existsSync(join(ROOT, 'favicon.svg')));

console.log(`\n${passed} kontrol geçti, ${failures.length} hata.`);
if (failures.length) {
  console.log('\nHATALAR:');
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log('SONUÇ: Chrome "Uygulamayı yükle" ölçütleri sağlanıyor.');
