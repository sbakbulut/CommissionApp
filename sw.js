/* Commission Calculator — Service Worker
   Amaç: kurulu uygulamanın çevrimdışı açılması + tek dosyalık uygulamanın kabuğunu hızlı sunmak.
   ÖNEMLİ: HTML için "network-first" kullanılır; böylece her yeni sürüm (publish.sh push'u)
   internete bağlıyken ANINDA görünür, eski sürüm önbellekte takılı kalmaz.

   Strateji:
   - Gezinme (HTML) istekleri : network-first → ağ yoksa önbellekteki kabuk
   - Aynı kaynak statikler    : cache-first + arka planda tazeleme (stale-while-revalidate)
   - SheetJS (jsdelivr) CDN   : cache-first (çevrimdışı Excel dışa aktarma için; URL sürümlü)
   - Diğer çapraz kaynak      : karışmayız (tarayıcı normal davranır)
*/
'use strict';

const VERSION = 'v13.8';
const SHELL = `commission-shell-${VERSION}`;
const RUNTIME = `commission-runtime-${VERSION}`;
const CDN_HOSTS = ['cdn.jsdelivr.net', 'cdnjs.cloudflare.com', 'unpkg.com'];

/* Önbelleğe alınacak kabuk dosyaları (göreli yollar → her barındırma yolunda çalışır). */
const SHELL_ASSETS = [
  './',
  './index.html',
  './manifest.json',
  './favicon.svg',
  './icons/icon-192.png',
  './icons/icon-512.png',
  './icons/icon-maskable-192.png',
  './icons/icon-maskable-512.png',
  './icons/apple-touch-icon.png',
];

self.addEventListener('install', (event) => {
  event.waitUntil((async () => {
    const cache = await caches.open(SHELL);
    // Tek bir dosya 404 olsa bile kurulum çökmemeli → her isteği ayrı yakala.
    await Promise.all(SHELL_ASSETS.map((url) =>
      cache.add(new Request(url, { cache: 'reload' })).catch(() => undefined)
    ));
    await self.skipWaiting();
  })());
});

self.addEventListener('activate', (event) => {
  event.waitUntil((async () => {
    const keys = await caches.keys();
    await Promise.all(
      keys.filter((k) => k.startsWith('commission-shell-') || k.startsWith('commission-runtime-'))
          .filter((k) => k !== SHELL && k !== RUNTIME)
          .map((k) => caches.delete(k))
    );
    await self.clients.claim();
  })());
});

/* Yeni sürüm hazır olduğunda sayfa isterse hemen devreye alır: */
self.addEventListener('message', (event) => {
  if (event.data === 'SKIP_WAITING') self.skipWaiting();
});

async function networkFirst(request, fallbackUrl) {
  const cache = await caches.open(SHELL);
  try {
    const fresh = await fetch(request);
    if (fresh && fresh.status === 200 && fresh.type === 'basic') {
      cache.put(fallbackUrl, fresh.clone()).catch(() => undefined);
    }
    return fresh;
  } catch (err) {
    const cached = (await cache.match(fallbackUrl)) ||
                   (await cache.match(request, { ignoreSearch: true })) ||
                   (await cache.match('./index.html'));
    if (cached) return cached;
    throw err;
  }
}

async function staleWhileRevalidate(request, cacheName) {
  const cache = await caches.open(cacheName || SHELL);
  const cached = await cache.match(request, { ignoreSearch: true });

  const network = fetch(request).then((response) => {
    // opaque yanıtlar (no-cors CDN script) da önbelleğe alınabilir
    if (response && (response.status === 200 || response.type === 'opaque')) {
      cache.put(request, response.clone()).catch(() => undefined);
    }
    return response;
  }).catch(() => null);

  if (cached) return cached;                 // hızlı yol: önbellekten sun, ağ arka planda tazeler
  const fresh = await network;
  if (fresh) return fresh;
  return new Response('', { status: 504, statusText: 'Offline' });
}

self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;                       // POST vb. asla önbelleğe girmez
  if (request.headers.has('range')) return;                   // kısmi indirmeler karışmasın
  const url = new URL(request.url);

  if (url.origin === self.location.origin) {
    if (request.mode === 'navigate') {
      event.respondWith(networkFirst(request, './index.html'));
      return;
    }
    if (url.pathname.endsWith('/sw.js')) return;              // SW kendini önbelleğe almasın
    event.respondWith(staleWhileRevalidate(request, SHELL));
    return;
  }

  if (CDN_HOSTS.includes(url.hostname)) {
    event.respondWith(staleWhileRevalidate(request, RUNTIME));
  }
  // diğer çapraz kaynak istekler: dokunulmaz
});
