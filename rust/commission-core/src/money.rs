//! Para (`Money`) ve oran (`Rate`) tipleri.
//!
//! Ana fikir: parayı **asla** `f64` ile tutma.
//! - `Money` = cent cinsinden tam sayı (`i64`). 12,34 € → `1234`.
//! - `Rate`  = baz puan cinsinden tam sayı. 1 baz puan = %0,01. %1,5 → `150`.
//!
//! Böylece `101 € × %1,5` hesabı tam olarak `1,515 €` olur ve doğru
//! yuvarlanır (1,52 €). JavaScript'te veya Rust `f64` ile sonuç `1,51 €` olur.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Sub};

/// Euro cent cinsinden para miktarı.
///
/// `Money` ile `f64` birbirine karışamaz: derleyici buna izin vermez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_cents(cents: i64) -> Money {
        Money(cents)
    }

    pub const fn from_euros(euros: i64) -> Money {
        Money(euros * 100)
    }

    pub const fn cents(self) -> i64 {
        self.0
    }

    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Kullanıcı veya Excel metnini paraya çevirir.
    ///
    /// `index.html` içindeki `parseAmount()` ile aynı kuralları uygular:
    /// - `€ $ £ ¥ ₺` ve boşlukları siler.
    /// - `1.234,56` (Avrupa) ve `1,234.56` (ABD) biçimlerini tanır.
    /// - Yalnız virgül varsa: `1,234` → 1234 (binlik), `12,5` → 12,50 (ondalık).
    /// - 2'den fazla ondalık basamağı yarım yukarı yuvarlar.
    ///
    /// Fark: JavaScript geçersiz metni sessizce `0` yapar. Burada hata döner.
    pub fn parse(input: &str) -> Result<Money, ParseMoneyError> {
        let cleaned: String = input
            .chars()
            .filter(|c| !matches!(c, '€' | '$' | '£' | '¥' | '₺') && !c.is_whitespace())
            .collect();

        if cleaned.is_empty() || cleaned == "-" {
            return Ok(Money::ZERO);
        }

        let normalized = match (cleaned.rfind(','), cleaned.rfind('.')) {
            // İkisi de var: sonda olan ondalık ayıracıdır.
            (Some(comma), Some(dot)) if comma > dot => cleaned.replace('.', "").replace(',', "."),
            (Some(_), Some(_)) => cleaned.replace(',', ""),
            // Yalnız virgül var.
            (Some(_), None) => {
                let parts: Vec<&str> = cleaned.split(',').collect();
                if parts.len() == 2 && parts[1].len() == 3 {
                    cleaned.replace(',', "") // 1,234 → binlik ayıracı
                } else {
                    cleaned.replacen(',', ".", 1) // 12,5 → ondalık
                }
            }
            _ => cleaned.clone(),
        };

        decimal_to_cents(&normalized)
            .map(Money)
            .ok_or_else(|| ParseMoneyError(input.to_string()))
    }
}

/// `"-1234.567"` gibi bir metni cent'e çevirir (yarım yukarı).
fn decimal_to_cents(s: &str) -> Option<i64> {
    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let (int_part, frac_part) = digits.split_once('.').unwrap_or((digits, ""));

    let all_digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
    if (int_part.is_empty() && frac_part.is_empty())
        || !all_digits(int_part)
        || !all_digits(frac_part)
    {
        return None;
    }

    let euros: i64 = if int_part.is_empty() {
        0
    } else {
        int_part.parse().ok()?
    };
    let frac = frac_part.as_bytes();
    let digit = |i: usize| frac.get(i).map_or(0, |b| i64::from(b - b'0'));

    let mut cents = euros
        .checked_mul(100)?
        .checked_add(digit(0) * 10 + digit(1))?;
    if digit(2) >= 5 {
        cents += 1; // yarım yukarı
    }
    Some(if negative { -cents } else { cents })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseMoneyError(pub String);

impl fmt::Display for ParseMoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "geçersiz tutar: {:?}", self.0)
    }
}

impl std::error::Error for ParseMoneyError {}

/// `€1.234,56` biçiminde yazar (uygulamadaki `formatEUR()` ile aynı).
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        let euros = (abs / 100).to_string();

        // Binlik ayıracı: her 3 basamakta bir nokta.
        let mut grouped = String::new();
        for (i, ch) in euros.chars().enumerate() {
            if i > 0 && (euros.len() - i).is_multiple_of(3) {
                grouped.push('.');
            }
            grouped.push(ch);
        }
        let text = format!("{sign}€{grouped},{:02}", abs % 100);
        f.pad(&text)
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        self.0 += rhs.0;
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, Add::add)
    }
}

impl<'a> Sum<&'a Money> for Money {
    fn sum<I: Iterator<Item = &'a Money>>(iter: I) -> Money {
        iter.copied().sum()
    }
}

/// Komisyon oranı, baz puan cinsinden (1 bp = %0,01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rate(i64);

impl Rate {
    const BP_PER_ONE: i64 = 10_000;

    /// `Rate::from_bp(150)` = %1,5
    pub const fn from_bp(bp: i64) -> Rate {
        Rate(bp)
    }

    pub const fn bp(self) -> i64 {
        self.0
    }

    /// Oranı tutara uygular ve sonucu cent'e yuvarlar.
    ///
    /// Yuvarlama: yarım yukarı (sıfırdan uzağa). 1,515 → 1,52; -1,515 → -1,52.
    /// Ara hesap `i128` ile yapılır, taşma (overflow) olmaz.
    pub fn apply(self, amount: Money) -> Money {
        self.checked_apply(amount)
            .expect("komisyon i64 sınırını aştı")
    }

    /// `apply` ile aynı, ama sonuç `i64` sınırını aşarsa panik yerine `None` döner.
    pub fn checked_apply(self, amount: Money) -> Option<Money> {
        let product = i128::from(amount.0) * i128::from(self.0);
        let divisor = i128::from(Self::BP_PER_ONE);
        let (quotient, remainder) = (product / divisor, product % divisor);
        let rounded = if remainder.abs() * 2 >= divisor {
            quotient + product.signum()
        } else {
            quotient
        };
        i64::try_from(rounded).ok().map(Money)
    }
}

/// Döviz kuru, 4 ondalık basamak (TCMB biçimi). `36,1234` → `361234`.
///
/// Matematik olarak bir orandır: `Rate` ile aynı birim (1/10.000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FxRate(Rate);

impl FxRate {
    /// `FxRate::from_ten_thousandths(361_234)` = 36,1234
    pub const fn from_ten_thousandths(value: i64) -> FxRate {
        FxRate(Rate::from_bp(value))
    }

    /// Tutarı çevirir ve cent'e yuvarlar (yarım yukarı).
    pub fn convert(self, amount: Money) -> Option<Money> {
        self.0.checked_apply(amount)
    }
}

/// `1.50%` biçiminde yazar.
impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = format!("{}.{:02}%", self.0 / 100, self.0 % 100);
        f.pad(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eur(s: &str) -> Money {
        Money::parse(s).unwrap()
    }

    #[test]
    fn parse_formats_like_the_web_app() {
        assert_eq!(eur("1.234,56"), Money::from_cents(123_456));
        assert_eq!(eur("1,234.56"), Money::from_cents(123_456));
        assert_eq!(eur("€ 31.000,00"), Money::from_euros(31_000));
        assert_eq!(eur("1,234"), Money::from_euros(1_234)); // binlik
        assert_eq!(eur("12,5"), Money::from_cents(1_250)); // ondalık
        assert_eq!(eur("101"), Money::from_euros(101));
        assert_eq!(eur("₺-5,25"), Money::from_cents(-525));
        assert_eq!(eur(""), Money::ZERO);
        assert_eq!(eur("-"), Money::ZERO);
    }

    #[test]
    fn parse_rounds_extra_decimals_half_up() {
        assert_eq!(eur("1.005"), Money::from_cents(101));
        assert_eq!(eur("1.004"), Money::from_cents(100));
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(Money::parse("abc").is_err());
        assert!(Money::parse("12a").is_err());
        assert!(Money::parse("1.2.3").is_err());
    }

    #[test]
    fn display_matches_format_eur() {
        assert_eq!(Money::from_cents(123_456).to_string(), "€1.234,56");
        assert_eq!(Money::from_cents(5).to_string(), "€0,05");
        assert_eq!(Money::from_euros(1_000_000).to_string(), "€1.000.000,00");
        assert_eq!(Money::from_cents(-152).to_string(), "-€1,52");
    }

    /// JavaScript'te yanlış çıkan örnekler (bkz. index.html `n2()` / `toFixed`).
    #[test]
    fn rate_rounds_half_up_where_float_fails() {
        let icon = Rate::from_bp(150);
        let silver = Rate::from_bp(50);
        assert_eq!(icon.apply(Money::from_euros(101)), Money::from_cents(152)); // JS: 1,51
        assert_eq!(icon.apply(Money::from_euros(105)), Money::from_cents(158)); // JS: 1,57
        assert_eq!(silver.apply(Money::from_euros(117)), Money::from_cents(59)); // JS: 0,58
        assert_eq!(silver.apply(Money::from_euros(119)), Money::from_cents(60)); // JS: 0,59
    }

    #[test]
    fn rate_handles_negative_amounts_symmetrically() {
        assert_eq!(
            Rate::from_bp(150).apply(Money::from_euros(-101)),
            Money::from_cents(-152)
        );
    }

    #[test]
    fn fx_conversion_rounds_half_up() {
        let fx = FxRate::from_ten_thousandths(361_234); // 36,1234
        // 1,52 € × 36,1234 = 54,907568 ₺ → 54,91 ₺
        assert_eq!(
            fx.convert(Money::from_cents(152)),
            Some(Money::from_cents(5_491))
        );
        // 0,50 € × 1,0001 = 0,50005 → 0,50
        let near_one = FxRate::from_ten_thousandths(10_001);
        assert_eq!(
            near_one.convert(Money::from_cents(50)),
            Some(Money::from_cents(50))
        );
        // Taşma panik yapmaz.
        assert_eq!(
            Rate::from_bp(i64::MAX).checked_apply(Money::from_cents(i64::MAX)),
            None
        );
    }

    #[test]
    fn rate_display() {
        assert_eq!(Rate::from_bp(150).to_string(), "1.50%");
        assert_eq!(Rate::from_bp(50).to_string(), "0.50%");
        assert_eq!(Rate::from_bp(300).to_string(), "3.00%");
    }
}
