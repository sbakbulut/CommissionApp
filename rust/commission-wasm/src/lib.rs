//! `commission-core` için WebAssembly arayüzü.
//!
//! Bu dosya `wasm32-unknown-unknown` için derlenir ve çıktı `index.html`
//! içine base64 olarak gömülür (bkz. `rust/build-wasm.sh`).
//!
//! Arayüz bilerek basittir: yalnız tam sayılar. `wasm-bindgen` gerekmez.
//! - Para  : cent (`i64`, JavaScript tarafında `BigInt`)
//! - Oran  : baz puan (`i64`, 1 bp = %0,01)
//! - Kur   : 1/10.000 birim (`i64`, 36,1234 → 361234)
//! - Kategori: 0 silver · 1 icon · 2 others · 3 watches (`CATS` sırası)
//! - Mantıksal değer: `u32` (0 = hayır, 1 = evet)
//!
//! Hata durumunda `ERROR` (= `i64::MIN`) döner. Kod asla panik yapmaz.

use commission_core::{Category, FxRate, Money, Rules};

/// Hata değeri. Gerçek bir tutar bu değere ulaşamaz.
pub const ERROR: i64 = i64::MIN;

/// Arayüz sürümü. JavaScript bu değeri kontrol eder.
pub const API_VERSION: u32 = 1;

#[unsafe(no_mangle)]
pub extern "C" fn api_version() -> u32 {
    API_VERSION
}

/// Bir satırın oranı (baz puan). Geçersiz kategori → `ERROR`.
#[unsafe(no_mangle)]
pub extern "C" fn rate_bp(category: u32, amount_cents: i64, target_met: u32) -> i64 {
    match Category::from_index(category) {
        Some(c) => Rules::default()
            .rate_for(c, Money::from_cents(amount_cents), target_met != 0)
            .bp(),
        None => ERROR,
    }
}

/// Bir satırın komisyonu (cent, yarım yukarı yuvarlanmış).
#[unsafe(no_mangle)]
pub extern "C" fn commission_cents(category: u32, amount_cents: i64, target_met: u32) -> i64 {
    let Some(c) = Category::from_index(category) else {
        return ERROR;
    };
    let amount = Money::from_cents(amount_cents);
    Rules::default()
        .rate_for(c, amount, target_met != 0)
        .checked_apply(amount)
        .map_or(ERROR, Money::cents)
}

/// Hedef tuttu mu? Hedef ≤ 0 ise "ayarlanmadı" sayılır → 0.
#[unsafe(no_mangle)]
pub extern "C" fn target_met(total_sales_cents: i64, target_cents: i64) -> u32 {
    u32::from(target_cents > 0 && total_sales_cents >= target_cents)
}

/// EUR → TRY çevirisi (cent, yarım yukarı yuvarlanmış).
#[unsafe(no_mangle)]
pub extern "C" fn convert_cents(amount_cents: i64, fx_ten_thousandths: i64) -> i64 {
    FxRate::from_ten_thousandths(fx_ten_thousandths)
        .convert(Money::from_cents(amount_cents))
        .map_or(ERROR, Money::cents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_match_core() {
        assert_eq!(api_version(), 1);
        assert_eq!(rate_bp(1, 10_100, 0), 150);
        assert_eq!(rate_bp(1, 10_100, 1), 300);
        assert_eq!(rate_bp(2, 3_100_000, 1), 200);
        assert_eq!(commission_cents(1, 10_100, 0), 152);
        assert_eq!(commission_cents(0, 11_700, 0), 59);
        assert_eq!(target_met(100, 100), 1);
        assert_eq!(target_met(99, 100), 0);
        assert_eq!(target_met(100, 0), 0);
        assert_eq!(convert_cents(152, 361_234), 5_491);
    }

    #[test]
    fn errors_do_not_panic() {
        assert_eq!(rate_bp(9, 100, 0), ERROR);
        assert_eq!(commission_cents(9, 100, 0), ERROR);
        assert_eq!(convert_cents(i64::MAX, i64::MAX), ERROR);
    }
}
