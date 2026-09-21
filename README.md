# CommissionApp — Satış Komisyon Hesaplayıcı

Tek dosyalık satış komisyon hesaplayıcı. **Canlı (GitHub Pages):** https://sbakbulut.github.io/CommissionApp/

Kurulum gerekmez — `index.html` dosyasını tarayıcıda aç (veya yukarıdaki canlı adresi kullan), verilerini gir, Excel olarak dışa aktar.
Veriler tarayıcının `localStorage`'ında tutulur; hiçbir sunucuya gönderilmez.

## Kullanım

1. https://sbakbulut.github.io/CommissionApp/ adresini aç (mobilde de çalışır)
2. **Currency** seçicisini `TRY` yap (TL raporu için)
3. Satışları elle gir veya Excel/CSV/HTML dosyasından içe aktar
4. **Export Excel** → `commission-report-TRY-YYYY-MM-DD.xlsx`

## TRY (₺) kur dönüşümü — v13.6

Satış tutarları EUR ise TL karşılığı **her satırın KENDİ AYININ** TCMB döviz satış kuru ile hesaplanır:

| Konu | Davranış |
|---|---|
| Kur kaynağı | TCMB (`kurlar/YYYYMM/DDMMYYYY.xml`) — Cloudflare worker proxy + 5 yedek proxy |
| Hedef gün | Ayın **son iş günü** (hafta sonu/tatil atlanır) |
| Ay henüz bitmediyse | Bugünden geriye ilk yayımlanmış gün + Excel'de `[ay sürüyor]` etiketi |
| O gün yayım yok (404) | En fazla 15 gün geriye taranır |
| Ağ hatası/timeout | **Kur uydurulmaz** — net hata verilir (`TCMB kuru alınamadı…`) |
| Rapor şeffaflığı | Excel'de `FX Source` satırı + her satırda `EUR/TRY` kolonu |
| Arayüz | `💱 TRY kuru` kutusu kullanılan kurları gösterir |

### v13.6'da düzeltilen hatalar

1. **Ay bazlı kur yoktu:** tüm rapor tek (rapor ayının) kuruyla çevriliyordu → karışık ay dosyalarında yanlış TL.
2. **Sessiz hatalı kur:** kur isteği 5 sn'de zaman aşımına uğrayınca kod sessizce bir önceki iş gününün kuruna düşüyordu.
   Örnek: 31.08.2026 (55,9658) alınamayınca 28.08.2026 (56,0853) kullanılıyordu → 10.000 € için 1.195 TL fazla.
   Artık istek 12 sn + 3 deneme; hata sessizce yutulmaz.
3. **Yanlış yıl / gelecek tarihli satır** içe aktarımda uyarı verir.

## Kullanım (geliştirme / yayın)

```bash
# Yerel dosyayı düzenledikten sonra repoyu güncelle:
./publish.sh                      # /sdcard'daki dosyayı index.html'e kopyalar, commit'ler, push eder
./publish.sh "özel commit mesajı"
```

## Sürüm

- `v13.6` — ay bazlı TCMB kuru, ağ hatası ayrımı, `[ay sürüyor]` etiketi, `💱 TRY kuru` kutusu, gelecek tarih uyarısı
- `v13.4` — önceki sürüm (repodaki ilk `index.html`)

## Not

SheetJS (XLSX) ve Chart.js CDN'den yüklenir; internet yoksa grafik/Excel dışa aktarma çalışmaz.
