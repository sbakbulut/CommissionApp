# CommissionApp — Satış Komisyon Hesaplayıcı

Tek dosyalık satış komisyon hesaplayıcı. **Canlı (GitHub Pages):** https://sbakbulut.github.io/CommissionApp/

Kurulum gerekmez — `index.html` dosyasını tarayıcıda aç (veya yukarıdaki canlı adresi kullan), verilerini gir, Excel olarak dışa aktar.
Veriler tarayıcının `localStorage`'ında tutulur; hiçbir sunucuya gönderilmez.

Telefona **gerçek uygulama (PWA)** olarak kurulabilir: bkz. [Telefona uygulama olarak kurma](#telefona-uygulama-olarak-kurma-pwa).

## Kullanım

1. https://sbakbulut.github.io/CommissionApp/ adresini aç (mobilde de çalışır)
2. **Currency** seçicisini `TRY` yap (TL raporu için)
3. Satışları elle gir veya Excel/CSV/HTML dosyasından içe aktar
4. **Export Excel** → `commission-report-TRY-YYYY-MM-DD.xlsx`

## Telefona uygulama olarak kurma (PWA)

Uygulama Chrome'un **kurulabilir web uygulaması** ölçütlerini sağlar (`v13.7`'den itibaren).

### Android / Chrome (veya Edge)

1. https://sbakbulut.github.io/CommissionApp/ adresini Chrome'da aç
2. Sayfada 30 saniye kal ve bir kez dokun (Chrome'un "kullanıcı etkileşimi" ölçütü)
3. Sağ altta çıkan **`⬇ Install app`** düğmesine dokun
   *(düğme çıkmazsa: Chrome menüsü `⋮` → **Uygulamayı yükle** / **Ana ekrana ekle**)*
4. Kurulumdan sonra uygulama ayrı bir simge olarak açılır — tarayıcı çubuğu olmadan (standalone), çevrimdışı da açılır

### iOS / Safari

1. Adresi Safari'de aç → **Paylaş** (□↑) → **Ana Ekrana Ekle**
   *(iOS'ta Chrome kurulum API'si yoktur; seçenek yalnız Safari'de çıkar)*

### Kurulum seçeneği görünmüyorsa

| Olası neden | Çözüm |
|---|---|
| Eski bir "Ana ekrana ekle" kısayolu zaten var (Chrome bunu "kurulu" sayar) | Ana ekrandaki eski kısayolu sil, sayfayı yenile |
| Chrome manifest'i önbelleğe aldı | Chrome → Ayarlar → Gizlilik → **Site verilerini temizle** (yalnız bu site) veya sekmeyi tamamen kapatıp yeniden aç |
| Sayfa `http://` veya `file://` üzerinden açık | Kurulum yalnız `https://` üzerinde çalışır → GitHub Pages adresini kullan |
| 30 saniye/dokunma ölçütü dolmadı | Sayfada biraz bekle, kaydır, bir kez dokun |

Teşhis: masaüstü Chrome'da uygulamayı açıp konsolda `await commissionPwa()` yazın —
manifest bağlantısı, service worker durumu, standalone modu ve kurulum isteğinin yakalanıp yakalanmadığını listeler.

### Bu ölçütler için eklenen dosyalar

| Dosya | Görev |
|---|---|
| `manifest.json` | `name`, `short_name`, `start_url`, `scope`, `display: standalone`, `theme_color` ve **192px + 512px** ikonlar (zorunlu alanlar) |
| `sw.js` | Service worker: HTML **network-first** (yeni sürüm anında görünür), statikler cache-first, SheetJS CDN önbelleği → çevrimdışı açılış |
| `icons/*.png` | 192/512 `any` + 192/512 `maskable` (Android uyarlanır simge) + 180px `apple-touch-icon` |
| `favicon.svg` | Sekme simgesi (marka: Tiffany mavisi + yükselen sütunlar) |
| `pwa/head.html`, `pwa/body-end.html` | `index.html`'e gömülen PWA bloklarının **kaynak kopyası** (publish.sh enjekte eder) |
| `tools/make-icons.py` | İkonları bağımlılık olmadan üretir/denetler (PIL/ImageMagick gerekmez) |
| `tools/check-pwa.mjs` | Kurulabilirlik ölçütlerini ve blok senkronizasyonunu denetler (57 kontrol) |
| `tools/check-fx.mjs` | TRY kur çevrimini denetler: rapor ayı, son iş günü, **tek kur** (ağ erişimi gerekmez) |

> `index.html` tek dosyalık kalmaya devam eder: PWA etiketleri ve kurulum düğmesi dosyanın içine gömülüdür.

## TRY (₺) kur dönüşümü — v13.8 (rapor ayı son iş günü, **TEK** kur)

Satış tutarları EUR ise TL karşılığı **raporun tamamı için tek kurla** hesaplanır:
raporda geçen **en son satış ayının (rapor ayı)** TCMB döviz satış kuru, tüm satırlara
— hangi aya/güne ait olursa olsun — aynı şekilde uygulanır.

| Konu | Davranış |
|---|---|
| Kur kaynağı | TCMB (`kurlar/YYYYMM/DDMMYYYY.xml`) — Cloudflare worker proxy + 5 yedek proxy |
| Kullanılan gün | Rapor ayının **son iş günü** (hafta sonu/tatil atlanır) |
| Rapor ayı | Dosyadaki **en son** satış ayı (örn. Ağustos + Eylül satırları varsa Eylül) |
| Uygulama alanı | **Tüm satırlar aynı kuru kullanır** — satır/ay/gün başına ayrı kur yoktur |
| Ay henüz bitmediyse | Bugünden geriye ilk yayımlanmış gün + Excel'de `[ay sürüyor]` etiketi |
| O gün yayım yok (404) | En fazla 15 gün geriye taranır |
| Ağ hatası/timeout | **Kur uydurulmaz** — net hata verilir (`TCMB kuru alınamadı…`) |
| Rapor şeffaflığı | Excel'de `FX Source` satırı rapor ayını + kullanılan tek kuru yazar; `EUR/TRY` kolonları aynı kuru gösterir |
| Arayüz | `💱 TRY kuru` kutusu kullanılan tek kuru gösterir |

### Kur davranışının sürüm geçmişi

| Sürüm | Davranış |
|---|---|
| v13.4 | Satır başına **günlük** kur (her gün kendi kuruyla → satırlar farklı kur) |
| v13.5 | Tek kur — raporun son ayının son iş günü (`fxSingleRate`) |
| v13.6 | **Ay bazlı** kur — her satır kendi ayının kuruyla (karışık ay raporunda birden fazla kur) |
| **v13.8** | **Tek kur — rapor ayının son iş günü** (tüm satırlara aynı kur; ay bazlı davranış kaldırıldı) |

### v13.8'de değişenler

1. **Ay bazlı kur kaldırıldı:** karışık ay dosyalarında (örn. Ağustos + Eylül satırları) satırlar
   farklı kurlarla çevriliyordu → artık rapor genelinde **tek** kur kullanılır (rapor ayı = en son satış ayı).
2. **v13.6'dan korunan düzeltmeler:** sessiz hatalı kur (zaman aşımında bir önceki iş gününün kuruna düşme)
   giderildi — örnek: 31.08.2026 (55,9658) alınamayınca 28.08.2026 (56,0853) kullanılıyordu → 10.000 € için
   1.195 TL fazla; yanlış yıl / gelecek tarihli satırlar içe aktarımda uyarı verir.

## Kullanım (geliştirme / yayın)

```bash
# Yerel dosyayı düzenledikten sonra repoyu güncelle:
./publish.sh                      # /sdcard'daki dosyayı index.html'e kopyalar, PWA bloklarını enjekte eder, commit'ler, push eder
./publish.sh "özel commit mesajı"
./publish.sh --no-push "deneme"   # sadece commit, push yok
```

`publish.sh` kopyalamadan sonra:

1. `pwa/head.html` ve `pwa/body-end.html` bloklarını `index.html`'e **idempotent** olarak enjekte eder
   (işaret: `PWA:HEAD:START` / `PWA:BODY:START`) — böylece telefondaki kaynak dosya PWA etiketlerini ezse bile
   kurulabilirlik korunur,
2. gömülü JS'i `node --check` ile sözdizimi denetiminden geçirir (bozuksa commit etmez).

Yayın öncesi kurulabilirlik denetimi:

```bash
node tools/check-pwa.mjs          # 57 kontrol: manifest, ikonlar, meta etiketler, sw.js, blok senkronizasyonu
node tools/check-fx.mjs           # TRY kur çevrimi: rapor ayı + son iş günü + TEK kur (ay/gün bazlı kur olmadığı)
python3 tools/make-icons.py       # ikonları yeniden üretir ve denetler (opsiyonel)
python3 -m http.server 8080       # yerel test: http://localhost:8080
```

## Sürüm

- `v13.8` — **TRY çevrimi TEK kur**: rapor ayının (dosyadaki en son satış ayı) son iş günü TCMB kuru
  tüm satırlara uygulanır; ay bazlı/gün bazlı kur kaldırıldı, `tools/check-fx.mjs` eklendi
- `v13.7` — **PWA kurulumu**: `manifest.json` (192/512 ikon), `sw.js` (network-first + çevrimdışı kabuk), `icons/`,
  tema/ikon meta etiketleri, `⬇ Install app` düğmesi, `commissionPwa()` teşhis yardımcısı,
  `tools/check-pwa.mjs` + `tools/make-icons.py`
- `v13.6` — ay bazlı TCMB kuru (v13.8'de tek kura döndü), ağ hatası ayrımı, `[ay sürüyor]` etiketi, `💱 TRY kuru` kutusu, gelecek tarih uyarısı
- `v13.4` — önceki sürüm (repodaki ilk `index.html`)

## Not

SheetJS (XLSX) CDN'den (jsdelivr) yüklenir; kurulum sonrası service worker bu dosyayı da önbelleğe alır,
böylece **daha önce çevrimiçi açılmış bir kurulum çevrimdışı da açılır ve Excel dışa aktarma çalışır.**
Hiç çevrimiçi açılmamış taze bir kurulumda çevrimdışı Excel dışa aktarma için önce bir kez internete bağlanmak gerekir.
