//! CommissionApp komisyon kuralları, Rust ile.
//!
//! Kurallar `index.html` ile aynıdır:
//! - Kategori oranları: Silver %0,5 · Icon %1,5 · Others %1 · Watches %1,5
//! - Toplam satış ≥ hedef ise: Icon ve Watches %3 olur.
//! - Hedef tuttuysa: Others satırı ≥ €31.000 ise %2 olur.
//!
//! Web uygulamasından farkları (bilerek):
//! 1. Para cent ile tam sayı olarak tutulur → kuruş hatası yok.
//! 2. Her satır ayrı yuvarlanır. Toplam = yuvarlanmış satırların toplamı.
//!    Böylece Excel'deki TOTAL satırı, detay satırlarının toplamına eşit olur.

pub mod money;

use std::fmt;
use std::str::FromStr;

pub use money::{Money, ParseMoneyError, Rate};

/// Ürün kategorisi. `enum` olduğu için yazım hatası derleme anında yakalanır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Silver,
    Icon,
    Others,
    Watches,
}

impl Category {
    pub const ALL: [Category; 4] = [
        Category::Silver,
        Category::Icon,
        Category::Others,
        Category::Watches,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Category::Silver => "Silver",
            Category::Icon => "Icon",
            Category::Others => "Others",
            Category::Watches => "Watches",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.label())
    }
}

impl FromStr for Category {
    type Err = String;

    fn from_str(s: &str) -> Result<Category, String> {
        match s.trim().to_lowercase().as_str() {
            "silver" => Ok(Category::Silver),
            "icon" => Ok(Category::Icon),
            "others" | "other" => Ok(Category::Others),
            "watches" | "watch" => Ok(Category::Watches),
            other => Err(format!(
                "bilinmeyen kategori: {other:?} (silver/icon/others/watches)"
            )),
        }
    }
}

/// Dört kategorinin oranları.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateTable {
    pub silver: Rate,
    pub icon: Rate,
    pub others: Rate,
    pub watches: Rate,
}

impl RateTable {
    pub const fn get(&self, category: Category) -> Rate {
        match category {
            Category::Silver => self.silver,
            Category::Icon => self.icon,
            Category::Others => self.others,
            Category::Watches => self.watches,
        }
    }
}

/// Tüm komisyon kuralları. `Rules::default()` web uygulamasının değerleridir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rules {
    pub base: RateTable,
    pub target_met: RateTable,
    pub high_value_threshold: Money,
    pub high_value_rate: Rate,
}

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            base: RateTable {
                silver: Rate::from_bp(50),
                icon: Rate::from_bp(150),
                others: Rate::from_bp(100),
                watches: Rate::from_bp(150),
            },
            target_met: RateTable {
                silver: Rate::from_bp(50),
                icon: Rate::from_bp(300),
                others: Rate::from_bp(100),
                watches: Rate::from_bp(300),
            },
            high_value_threshold: Money::from_euros(31_000),
            high_value_rate: Rate::from_bp(200),
        }
    }
}

impl Rules {
    /// Bir satıra uygulanacak oran (`getRateForRow()` ile aynı).
    pub fn rate_for(&self, category: Category, amount: Money, target_met: bool) -> Rate {
        if category == Category::Others && target_met && amount >= self.high_value_threshold {
            return self.high_value_rate;
        }
        if target_met {
            self.target_met.get(category)
        } else {
            self.base.get(category)
        }
    }
}

/// Bir satış satırı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sale {
    pub category: Category,
    pub amount: Money,
}

/// Hesaplanmış bir satır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line {
    pub sale: Sale,
    pub rate: Rate,
    pub commission: Money,
}

/// Kategori özeti.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategoryTotal {
    pub category: Category,
    pub count: usize,
    pub sales: Money,
    pub commission: Money,
}

/// Hesap sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub target: Option<Money>,
    pub target_met: bool,
    pub total_sales: Money,
    pub total_commission: Money,
    pub lines: Vec<Line>,
}

impl Report {
    pub fn by_category(&self) -> [CategoryTotal; 4] {
        Category::ALL.map(|category| {
            let lines = self.lines.iter().filter(|l| l.sale.category == category);
            CategoryTotal {
                category,
                count: lines.clone().count(),
                sales: lines.clone().map(|l| l.sale.amount).sum(),
                commission: lines.map(|l| l.commission).sum(),
            }
        })
    }
}

/// Ana hesap fonksiyonu.
///
/// `target`: `None` veya sıfır ise hedef "ayarlanmamış" sayılır (web uygulaması ile aynı).
pub fn calculate(sales: &[Sale], target: Option<Money>, rules: &Rules) -> Report {
    let total_sales: Money = sales.iter().map(|s| s.amount).sum();
    let target = target.filter(|t| t.is_positive());
    let target_met = target.is_some_and(|t| total_sales >= t);

    let lines: Vec<Line> = sales
        .iter()
        .map(|&sale| {
            let rate = rules.rate_for(sale.category, sale.amount, target_met);
            Line {
                sale,
                rate,
                commission: rate.apply(sale.amount),
            }
        })
        .collect();

    // Toplam = yuvarlanmış satırların toplamı → rapor her zaman tutarlı.
    let total_commission = lines.iter().map(|l| l.commission).sum();

    Report {
        target,
        target_met,
        total_sales,
        total_commission,
        lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sale(category: Category, amount: &str) -> Sale {
        Sale {
            category,
            amount: Money::parse(amount).unwrap(),
        }
    }

    #[test]
    fn base_rates_without_target() {
        let sales = [
            sale(Category::Silver, "1000"),
            sale(Category::Icon, "1000"),
            sale(Category::Others, "1000"),
            sale(Category::Watches, "1000"),
        ];
        let r = calculate(&sales, None, &Rules::default());
        assert!(!r.target_met);
        let c: Vec<i64> = r.lines.iter().map(|l| l.commission.cents()).collect();
        assert_eq!(c, [500, 1500, 1000, 1500]);
        assert_eq!(r.total_commission, Money::from_euros(45));
    }

    #[test]
    fn target_met_doubles_icon_and_watches() {
        let sales = [
            sale(Category::Icon, "20000"),
            sale(Category::Watches, "11000"),
        ];
        let r = calculate(&sales, Some(Money::from_euros(31_000)), &Rules::default());
        assert!(r.target_met);
        assert_eq!(r.lines[0].rate, Rate::from_bp(300));
        assert_eq!(r.lines[1].rate, Rate::from_bp(300));
        assert_eq!(r.total_commission, Money::from_euros(930));
    }

    #[test]
    fn high_value_others_only_when_target_met() {
        let big = sale(Category::Others, "31000");
        let rules = Rules::default();

        let not_met = calculate(&[big], Some(Money::from_euros(50_000)), &rules);
        assert_eq!(not_met.lines[0].rate, Rate::from_bp(100));

        let met = calculate(&[big], Some(Money::from_euros(31_000)), &rules);
        assert_eq!(met.lines[0].rate, Rate::from_bp(200));
        assert_eq!(met.lines[0].commission, Money::from_euros(620));

        let just_below = calculate(
            &[sale(Category::Others, "30999,99")],
            Some(Money::from_euros(1)),
            &rules,
        );
        assert_eq!(just_below.lines[0].rate, Rate::from_bp(100));
    }

    #[test]
    fn zero_target_counts_as_not_set() {
        let r = calculate(
            &[sale(Category::Icon, "100")],
            Some(Money::ZERO),
            &Rules::default(),
        );
        assert_eq!(r.target, None);
        assert!(!r.target_met);
    }

    /// JS: 0.1 on kez toplanınca 0.9999999999999999 olur ve hedefi kaçırır.
    #[test]
    fn exact_target_is_met_without_float_drift() {
        let sales = vec![sale(Category::Silver, "0,10"); 10];
        let r = calculate(&sales, Some(Money::from_euros(1)), &Rules::default());
        assert_eq!(r.total_sales, Money::from_euros(1));
        assert!(r.target_met);
    }

    /// JS: satırlar 105,17 € ama TOTAL 105,18 € çıkıyordu.
    #[test]
    fn total_equals_sum_of_rounded_lines() {
        let sales: Vec<Sale> = ["1001.5", "2003.5", "3005.5", "1001.5"]
            .iter()
            .map(|a| sale(Category::Icon, a))
            .collect();
        let r = calculate(&sales, None, &Rules::default());
        let line_sum: Money = r.lines.iter().map(|l| l.commission).sum();
        assert_eq!(r.total_commission, line_sum);
        assert_eq!(r.total_commission, Money::from_cents(10_517));
    }

    #[test]
    fn category_summary() {
        let sales = [
            sale(Category::Icon, "101"),
            sale(Category::Icon, "105"),
            sale(Category::Silver, "117"),
        ];
        let r = calculate(&sales, None, &Rules::default());
        let [silver, icon, others, _watches] = r.by_category();
        assert_eq!(
            (silver.count, silver.commission),
            (1, Money::from_cents(59))
        );
        assert_eq!(
            (icon.count, icon.sales, icon.commission),
            (2, Money::from_euros(206), Money::from_cents(310))
        );
        assert_eq!(others.count, 0);
    }

    #[test]
    fn category_parsing() {
        assert_eq!("ICON".parse::<Category>(), Ok(Category::Icon));
        assert_eq!(" watch ".parse::<Category>(), Ok(Category::Watches));
        assert!("gold".parse::<Category>().is_err());
    }
}
