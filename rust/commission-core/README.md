# commission-core (Rust)

CommissionApp komisyon kurallarının Rust sürümü. Deneme / öğrenme projesidir.
Web uygulaması (`index.html`) bunu **kullanmaz**.

## Neden?

Web uygulaması parayı JavaScript `number` (float) ile hesaplar. Bu yüzden bazı
satırlar 1 cent eksik çıkar. Örnek: `101 € × %1,5` → uygulama **1,51 €**, doğru **1,52 €**.

Bu kütüphane parayı **cent olarak tam sayı** (`i64`) tutar. Hata olmaz.

## Komutlar

```sh
cd rust/commission-core
cargo test                                         # 15 test
cargo run -- examples/sales.csv --target 30000     # örnek rapor
```

## Dosyalar

| Dosya | İçerik |
|---|---|
| `src/money.rs` | `Money` (cent) ve `Rate` (baz puan) tipleri, metin okuma, yuvarlama |
| `src/lib.rs` | Kategoriler, oranlar, hedef kuralı, `calculate()` |
| `src/main.rs` | CSV okuyan komut satırı programı |
| `examples/sales.csv` | Örnek veri |

## Web uygulamasından farklar

1. Yuvarlama: yarım yukarı (1,515 → 1,52).
2. TOPLAM = yuvarlanmış satırların toplamı.
3. Geçersiz tutar hata verir. Sessizce `0` olmaz.
