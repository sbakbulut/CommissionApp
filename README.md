# CommissionApp — Satış Komisyon Hesaplayıcı

Satış komisyon hesaplayıcı. **Canlı (GitHub Pages):** https://sbakbulut.github.io/CommissionApp/

Kurulum gerekmez — canlı adresi aç, verilerini gir, Excel olarak dışa aktar.
Uygulamanın tamamı **Rust** ile yazılmıştır ve tarayıcıda WebAssembly olarak çalışır (`v14.0`'dan itibaren).
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
./publish.sh                      # Rust testleri + WebAssembly derleme + PWA denetimi, sonra commit + push
./publish.sh "özel commit mesajı"
./publish.sh --no-push "deneme"   # sadece commit, push yok
```

Telefondaki `/sdcard/.../commission calculator.html` dosyası **artık kullanılmaz**. Kod `rust/` klasöründedir.

Bir kez kurulacaklar:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
```

Denetimler:

```bash
cargo test --manifest-path rust/Cargo.toml --workspace   # Rust birim testleri
node tools/check-parity.mjs       # eski JS ile Rust: ürün kodu, tutar, tarih, başlık, HTML (200.000+ girdi)
node tools/check-e2e.cjs          # eski ve yeni uygulama tarayıcıda yan yana (27 adım; playwright + xlsx gerekir)
node tools/check-pwa.mjs          # manifest, ikonlar, meta etiketler, sw.js, blok senkronizasyonu
python3 -m http.server 8080       # yerel test: http://localhost:8080  (file:// ile açılmaz — WebAssembly modülü)
```

## Yapı (Rust → WebAssembly)

| Yer | İçerik |
|---|---|
| `index.html` | Ekran (HTML + CSS) — v13.8 ile aynı görünüm. JS kodu yok, yalnız WebAssembly'i başlatan satır. |
| `rust/commission-app/` | Uygulamanın tamamı: ekran, ürün kodu → kategori, Excel okuma/yazma, TCMB kuru, takım raporu, kayıt |
| `rust/commission-core/` | Para (`Money`, cent) ve komisyon kuralları |
| `rust/commission-cli/` | Komut satırı: CSV hesabı ve Excel rapor denetimi |
| `pkg/` | Derlenmiş uygulama (`rust/build-web.sh` üretir, commit edilir) |
| `sw.js`, `pwa/` | Çevrimdışı çalışma ve telefona kurulum (tarayıcı kuralı gereği JavaScript) |

- Para **cent olarak tam sayı** ile hesaplanır (101 € × %1,5 = **1,52 €**).
- Kayıtlı veriler (`localStorage`) eski sürümle **aynı biçimdedir**; güncellemede satışlar kaybolmaz.
- Excel kütüphanesi (SheetJS) artık indirilmez; Excel okuma/yazma Rust içindedir. İlk açılıştan sonra
  uygulama **tamamen çevrimdışı** çalışır (TCMB kuru hariç).
- Kalan JavaScript: WebAssembly yükleyicisi (otomatik üretilir), `sw.js` ve kurulum düğmesi — tarayıcılar
  bunları JavaScript olarak ister.

Ayrıntılar: [`rust/README.md`](rust/README.md)

## Sürüm

- `v14.0` — **Uygulamanın tamamı Rust** (WebAssembly). Görünüm, hesaplar ve Excel dosyaları v13.8 ile aynı
  (`tools/check-e2e.cjs` ile doğrulandı). Düzeltmeler: satıcı sütunu olmayan Excel dosyası artık içe
  aktarılıyor; atlanan satırlar listesi görünüyor; Excel başlık renkleri uygulanıyor. SheetJS kaldırıldı.

- `v13.8` — **TRY çevrimi TEK kur**: rapor ayının (dosyadaki en son satış ayı) son iş günü TCMB kuru
  tüm satırlara uygulanır; ay bazlı/gün bazlı kur kaldırıldı, `tools/check-fx.mjs` eklendi
- `v13.7` — **PWA kurulumu**: `manifest.json` (192/512 ikon), `sw.js` (network-first + çevrimdışı kabuk), `icons/`,
  tema/ikon meta etiketleri, `⬇ Install app` düğmesi, `commissionPwa()` teşhis yardımcısı,
  `tools/check-pwa.mjs` + `tools/make-icons.py`
- `v13.6` — ay bazlı TCMB kuru (v13.8'de tek kura döndü), ağ hatası ayrımı, `[ay sürüyor]` etiketi, `💱 TRY kuru` kutusu, gelecek tarih uyarısı
- `v13.4` — önceki sürüm (repodaki ilk `index.html`)

## Not

Uygulama ilk açılışta yaklaşık 0,8 MB (sıkıştırılmış) WebAssembly dosyası indirir; service worker bunu
önbelleğe alır. Sonraki açılışlar ve Excel işlemleri internet olmadan da çalışır.
