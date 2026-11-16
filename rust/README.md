# Rust hesap çekirdeği

Uygulamanın komisyon hesabı Rust ile yazılmıştır. Rust kodu WebAssembly'e (WASM)
derlenir ve `index.html` içine gömülür. Tarayıcıdaki hesap bu kodla yapılır.

## Neden?

Eski hesap JavaScript `number` (float) kullanıyordu. Bazı satırlar 1 cent eksik çıkıyordu:

| Satış | Oran | Eski (float) | Yeni (Rust) |
|---|---|---|---|
| 101 € | %1,5 | 1,51 € | **1,52 €** |
| 117 € | %0,5 | 0,58 € | **0,59 €** |
| 250,50 € | %1 | 2,50 € | **2,51 €** |

Rust kodu parayı **cent olarak tam sayı** (`i64`) tutar. Yuvarlama: yarım yukarı.

## Paketler

| Paket | Görev | Dış bağımlılık |
|---|---|---|
| `commission-core` | `Money`, `Rate`, `FxRate`, kurallar, `calculate()` | Yok |
| `commission-wasm` | Tarayıcı arayüzü (yalnız tam sayılar, 2,6 KB) | Yok |
| `commission-cli` | `commission` komutu: CSV hesabı + Excel rapor denetimi | `calamine` |

## Komutlar

```sh
# Bir kez: WASM hedefini kur
rustup target add wasm32-unknown-unknown

cd rust
cargo test                       # tüm Rust testleri
./build-wasm.sh                  # WASM derle ve index.html içine göm
node ../tools/check-wasm.mjs     # WASM = JS yedeği = referans (48 milyon hesap)

# Hesap (CSV)
cargo run -p commission-cli -- commission-cli/examples/sales.csv --target 30000

# Denetim: uygulamanın ürettiği Excel raporunu yeniden hesapla
cargo run -p commission-cli -- ~/Downloads/rapor.xlsx
```

## Kural değişikliği yaparken

Oranlar **iki yerde** durur:

1. `commission-core/src/lib.rs` → `Rules::default()` (Rust, asıl hesap)
2. `index.html` → `baseRates`, `targetRates`, `highValueThreshold`, `highValueRate`
   (ekran metinleri ve WASM yüklenemezse kullanılan JS yedeği)

Adımlar:

1. İki yeri de değiştir.
2. `./build-wasm.sh` çalıştır.
3. `node ../tools/check-wasm.mjs` çalıştır. İki yer farklıysa bu kontrol hata verir.

Güvenlik: Sayfa açılırken Rust oranlarını JS oranları ile karşılaştırır. Farklıysa
WASM kullanılmaz ve JS yedeği çalışır (o da tam sayı ile hesaplar). Böylece yalnız
`index.html` içinde oran değiştirilirse yeni oran yine de geçerli olur.
Konsolda `WASM could not load ... rates differ` uyarısı görünür.

## Denetim çıktısı örneği (eski uygulamanın raporu)

```
  24  Silver          €117,00   0.50%        €0,59         €0,58 ✗
  27  Icon            €101,00   1.50%        €1,52         €1,51 ✗
  29  Icon          €1.001,50   1.50%       €15,02               ✓
  ...
Denetim: 12 satırdan 7 tanesi farklı. Toplam fark (Rust − uygulama): €0,07
```
