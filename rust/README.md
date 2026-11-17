# Rust kodu

CommissionApp'in tamamı Rust ile yazılmıştır. Tarayıcıda WebAssembly olarak çalışır.

## Paketler

| Paket | Görev |
|---|---|
| `commission-core` | `Money` (cent), `Rate` (baz puan), `FxRate`, komisyon kuralları. Dış bağımlılık yok. |
| `commission-app` | Web uygulaması. Saf modüller `cargo test` ile test edilir; `ui` modülü yalnız WebAssembly'de derlenir. |
| `commission-cli` | `commission` komutu: CSV hesabı + uygulamanın Excel raporunu yeniden hesaplayıp denetleme. |

`commission-app` modülleri:

| Modül | İçerik | Eski JS karşılığı |
|---|---|---|
| `tiffany.rs` | Ürün kodu → koleksiyon, materyal, kategori | `parseTiffanyCode`, `inferCategory` |
| `tables.rs` | SKU ve kısaltma tabloları (index.html v13.8'den otomatik üretildi) | `SKU_TO_COLLECTION` vb. |
| `import.rs` | Excel / HTML ".xls" okuma, başlık ve sütun bulma, tutar ve tarih okuma | `importExcelLocal`, `findHeaderRow`, … |
| `calc.rs` | Oran, komisyon, hedef, TL çevirisi | `CommissionCore` |
| `report.rs` | Excel rapor içerikleri (4 tür) | `buildExcelData`, `buildPersonSheetData`, … |
| `xlsx.rs` | .xlsx yazma (`rust_xlsxwriter`) | SheetJS `writeFile` |
| `fx.rs` | TCMB kur günü, etiketler, XML okuma | `fxTargetForMonth`, `parseTcmbXml`, … |
| `state.rs` | `localStorage` kaydı (eski biçimle aynı) | `loadState`, `saveState`, … |
| `js.rs` | JavaScript sayı/tarih/metin kurallarının birebir kopyası | `toFixed`, `toLocaleString`, `parseFloat`, `Date` |
| `ui.rs` | Ekran ve olaylar (`web-sys`) | ekran kodu |

## Komutlar

```sh
cd rust
cargo test --workspace                 # birim testleri
./build-web.sh                         # WebAssembly → ../pkg/
node ../tools/check-parity.mjs         # eski JS ile Rust aynı mı (200.000+ girdi)
node ../tools/check-e2e.cjs            # eski ve yeni uygulama tarayıcıda yan yana
cargo run -p commission-cli -- ~/Downloads/rapor.xlsx   # Excel raporunu denetle
```

## Kural değişikliği

Oranlar yalnız bir yerdedir: `commission-core/src/lib.rs` → `Rules::default()`.
Ekrandaki oran tablosu `index.html` içinde sabit metindir; oran değişirse onu da güncelle.
Sonra `./build-web.sh` çalıştır.

## Neden JS kurallarının kopyası (`js.rs`)?

Eski uygulama ile **aynı** sonucu vermek için. Örnekler:

- Ekranda `€1,01` (JS `toLocaleString`, 1.005'i yukarı yuvarlar) ama Excel'de `1` (JS `toFixed`).
- `31.02.2026` tarihi JS'te hata değildir: `03.03.2026` olur.
- `"12abc"` tutarı JS'te `12` olur.

`tools/check-parity.mjs` ve `tools/check-e2e.cjs` bu kuralların eski uygulamayla aynı olduğunu kanıtlar.
