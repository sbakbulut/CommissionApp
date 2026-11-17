//! Product code → collection, material and commission category.
//!
//! One-to-one port of `parseTiffanyCode()` and `inferCategory()` from the old
//! JavaScript app (v13.8). The order of every check is the same, because the
//! first match wins. `tools/check-parity.mjs` compares this code with the old
//! JavaScript on a large set of codes.

use std::sync::LazyLock;

use commission_core::Category;
use regex::Regex;

use crate::js::{self, JS_S};
use crate::tables::{CODE_FIXES, MATERIAL_CODES, PRIORITY_COLLECTIONS, SKU_TO_COLLECTION};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedCode {
    pub material: String,
    pub collection: String,
}

fn re(pattern: &str) -> Regex {
    Regex::new(&pattern.replace(r"\s", JS_S)).expect("valid regex")
}

/// JavaScript `\b` is ASCII-only.
const B: &str = r"(?-u:\b)";

static COMBINED: LazyLock<Regex> = LazyLock::new(|| re(&format!("{B}(PT|PL|950)18([YRW]){B}")));
static COMBINED_REV: LazyLock<Regex> = LazyLock::new(|| re(&format!("{B}18([YRW])(PT|PL|950){B}")));
static MAT18: LazyLock<Regex> = LazyLock::new(|| re("18([YRW])"));
static B925: LazyLock<Regex> = LazyLock::new(|| re(&format!("{B}925{B}")));
// JavaScript has /i here; `upper` has no lower-case ASCII letters, so it is not needed.
static WATCH: LazyLock<Regex> = LazyLock::new(|| re(&format!("{B}WTCH|{B}WATCH|{B}WATCHES")));
static TW: LazyLock<Regex> = LazyLock::new(|| re(&format!("{B}TW{B}")));
static SOLITAIRE: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"{B}[0-9]+\.[0-9]+\s+[D-Z]\s+(FL|IF|VVS[12]|VS[12]|SI[12]|I1){B}"
    ))
});
static PT18: LazyLock<Regex> = LazyLock::new(|| re(&format!("{B}PT18[YR]{B}")));
static PT18_REV: LazyLock<Regex> = LazyLock::new(|| re(&format!("{B}18[YR]PT{B}")));

/// One regex per keyword without a space (`\bT[\s\-]WIRE\b` etc.), in table order.
static PRIORITY: LazyLock<Vec<(Option<Regex>, &'static str, &'static str)>> = LazyLock::new(|| {
    PRIORITY_COLLECTIONS
        .iter()
        .map(|&(code, name)| {
            if code.contains(' ') {
                (None, code, name)
            } else {
                let body: String = code
                    .chars()
                    .map(|c| {
                        if c == '-' || js::is_js_space(c) {
                            r"[\s\-]".to_string()
                        } else {
                            regex::escape(&c.to_string())
                        }
                    })
                    .collect();
                (Some(re(&format!("{B}{body}{B}"))), code, name)
            }
        })
        .collect()
});

static INFER_WATCHES: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"{B}watches?{B}|{B}atlas{B}|{B}east\s*west{B}|{B}tiffany\s*eternity{B}|{B}union\s*square{B}|{B}tiffany\s*cocktail{B}"
    ))
});
static INFER_SILVER: LazyLock<Regex> = LazyLock::new(|| re(r"sterling\s*silver|925"));
static INFER_ICON: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"hardwear|knot|t by tiffany|{B}t1{B}|lock|sixteen"
    ))
});

const WATCH_CASE_SIZES: [&str; 14] = [
    "20", "21", "25", "27", "28", "29", "30", "33", "34", "35", "36", "38", "40", "42",
];
const MOVEMENT_TOKENS: [&str; 4] = ["AU", "QZ", "QTZ", "SOL"];

/// The old `lookupCollectionBySku(sku)`.
pub fn lookup_collection_by_sku(sku: &str) -> &'static str {
    let key = js::trim(sku);
    if key.is_empty() {
        return "";
    }
    SKU_TO_COLLECTION
        .binary_search_by(|(k, _)| k.cmp(&key))
        .map_or("", |i| SKU_TO_COLLECTION[i].1)
}

fn color_name(c: &str) -> &'static str {
    match c {
        "Y" => "Yellow",
        "R" => "Rose",
        _ => "White",
    }
}

/// `"16" … "STONE"` and `"RETURN" … "TIFFANY"` (JavaScript `/A.*B/` inside one token).
fn has_in_order(t: &str, first: &str, then: &str) -> bool {
    t.find(first)
        .is_some_and(|i| t[i + first.len()..].contains(then))
}

/// The old `parseTiffanyCode(code, sku)`.
pub fn parse_tiffany_code(code: &str, sku: &str) -> ParsedCode {
    if code.is_empty() {
        return ParsedCode::default();
    }
    let mut upper = js::trim(&code.to_uppercase()).to_string();
    for &(bad, good) in CODE_FIXES {
        if upper.contains(bad) {
            upper = upper.replace(bad, good);
        }
    }

    let sku_collection = lookup_collection_by_sku(sku);
    let tokens: Vec<&str> = upper
        .split(|c: char| js::is_js_space(c) || c == '-' || c == '/' || c == '.')
        .filter(|t| !t.is_empty())
        .collect();

    // ═══ MATERIAL ═══
    let mut materials: Vec<String> = Vec::new();
    let combined = COMBINED
        .captures(&upper)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .or_else(|| {
            COMBINED_REV
                .captures(&upper)
                .map(|c| (c[2].to_string(), c[1].to_string()))
        });
    if let Some((_plat, color)) = combined {
        materials.push("Platinum".into());
        materials.push(format!("18k {} Gold", color_name(&color)));
    }
    for t in &tokens {
        if let Some(&(_, m)) = MATERIAL_CODES.iter().find(|(k, _)| k == t)
            && !materials.iter().any(|x| x == m)
        {
            materials.push(m.to_string());
        }
    }
    for t in &tokens {
        if !materials.is_empty() {
            break;
        }
        // /^SS(?!ET|TH|IN|T|G|ER|IN)/ && t !== 'SST'
        if let Some(rest) = t.strip_prefix("SS")
            && !["ET", "TH", "IN", "T", "G", "ER"]
                .iter()
                .any(|p| rest.starts_with(p))
            && *t != "SST"
            && !materials.iter().any(|x| x == "Sterling Silver")
        {
            materials.push("Sterling Silver".into());
        }
    }
    if materials.is_empty()
        && let Some(c) = MAT18.captures(&upper)
    {
        materials.push(format!("18k {} Gold", color_name(&c[1])));
    }
    if materials.is_empty() && B925.is_match(&upper) {
        materials.push("Sterling Silver".into());
    }
    let material = materials.join(" + ");

    // ═══ COLLECTION ═══
    let mut collection = String::new();
    if WATCH.is_match(&upper) {
        collection = "Watches".into();
    } else if !sku_collection.is_empty() {
        collection = sku_collection.into();
    } else {
        for (regex, code, name) in PRIORITY.iter() {
            let hit = match regex {
                None => upper.contains(code),
                Some(r) => r.is_match(&upper),
            };
            if hit {
                collection = (*name).into();
                break;
            }
        }
        if collection.is_empty()
            && (tokens.contains(&"T")
                || (upper.starts_with('T') && upper[1..].starts_with(|c: char| c.is_ascii_digit()))
                || TW.is_match(&upper))
        {
            collection = "T by Tiffany".into();
        }
        if collection.is_empty() {
            // T1 not followed by a digit, and no "T1<digit>" anywhere
            let t1_ok = {
                let mut any = false;
                let mut digit_after = false;
                for (i, _) in upper.match_indices("T1") {
                    match upper[i + 2..].chars().next() {
                        Some(c) if c.is_ascii_digit() => digit_after = true,
                        _ => any = true,
                    }
                }
                any && !digit_after
            };
            for t in &tokens {
                let t: &str = t;
                if t.strip_prefix("HW").is_some_and(|r| !r.starts_with("IRE")) {
                    collection = "HardWear".into();
                    break;
                }
                if t.contains("KNOT") {
                    collection = "Knot".into();
                    break;
                }
                if (t.contains("HARDWEAR") || t.contains("HW")) && !t.contains("LK") {
                    collection = "HardWear".into();
                    break;
                }
                if t.contains("SIXTEEN")
                    || t.contains("SCHLUM")
                    || t.contains("16ST")
                    || has_in_order(t, "16", "STONE")
                {
                    collection = "Sixteen Stone".into();
                    break;
                }
                let lk = t.strip_prefix("LK").is_some_and(|r| !r.starts_with("BLT"));
                if (t.starts_with("LOCK") || lk) && js::len16(t) <= 8 {
                    collection = "Lock".into();
                    break;
                }
                if t.contains("PKSA") {
                    collection = "Lock".into();
                    break;
                }
                if ["BONE", "ELSA", "PERETTI", "FULLHT", "FULLHRT", "PUFF"]
                    .iter()
                    .any(|k| t.contains(k))
                {
                    collection = "Elsa Peretti".into();
                    break;
                }
                if has_in_order(t, "RETURN", "TIFFANY") || t == "RT" || t == "RTT" {
                    collection = "Return to Tiffany".into();
                    break;
                }
                let t_digit =
                    t.starts_with('T') && t[1..].starts_with(|c: char| c.is_ascii_digit());
                if t_digit
                    || t.contains("TWIRE")
                    || t.contains("TSMILE")
                    || t.contains("TSMI")
                    || t1_ok
                {
                    collection = "T by Tiffany".into();
                    break;
                }
                if t.contains("SPLIT") {
                    collection = "Tiffany 1837".into();
                    break;
                }
            }
            if collection.is_empty() && SOLITAIRE.is_match(&upper) {
                collection = "True Diamond".into();
            }
            if collection.is_empty() && (PT18.is_match(&upper) || PT18_REV.is_match(&upper)) {
                collection = "Sixteen Stone".into();
            }
            if collection.is_empty() {
                let extract_size = |t: &str| -> Option<String> {
                    let b = t.as_bytes();
                    if b.len() >= 2 && b[0].is_ascii_digit() && b[1].is_ascii_digit() {
                        return Some(t[..2].to_string());
                    }
                    let n = b.len();
                    if n >= 2 && b[n - 1].is_ascii_digit() && b[n - 2].is_ascii_digit() {
                        return Some(t[n - 2..].to_string());
                    }
                    None
                };
                let has_movement = tokens.iter().any(|t| MOVEMENT_TOKENS.contains(t));
                let has_case = tokens.iter().any(|t| {
                    extract_size(t).is_some_and(|s| WATCH_CASE_SIZES.contains(&s.as_str()))
                });
                if has_movement && has_case {
                    collection = "Watches".into();
                }
            }
        }
    }
    ParsedCode {
        material,
        collection,
    }
}

/// The old `inferCategory(collection, material)`.
pub fn infer_category(collection: &str, material: &str) -> Category {
    let m = material.to_lowercase();
    let c = collection.to_lowercase();
    if INFER_WATCHES.is_match(&c) {
        return Category::Watches;
    }
    if INFER_SILVER.is_match(&m) {
        return Category::Silver;
    }
    if INFER_ICON.is_match(&c) {
        return Category::Icon;
    }
    Category::Others
}

/// Code (+ SKU) → category, collection and material.
pub fn categorize(code: &str, sku: &str) -> (Category, ParsedCode) {
    let parsed = parse_tiffany_code(code, sku);
    (infer_category(&parsed.collection, &parsed.material), parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(code: &str, sku: &str) -> Category {
        categorize(code, sku).0
    }

    #[test]
    fn known_codes() {
        assert_eq!(cat("HW SM DBL LKBLT 18Y", ""), Category::Icon);
        assert_eq!(cat("SS RTT HRT TAG PDT", ""), Category::Silver);
        assert_eq!(cat("ST ATLAS AU STP TB 38", ""), Category::Watches);
        assert_eq!(cat("PT CU DI 1.02 H VS1", ""), Category::Others);
        assert_eq!(
            parse_tiffany_code("PT CU DI 1.02 H VS1", "").collection,
            "True Diamond"
        );
        assert_eq!(cat("18RHLFDIT1NRWHNGBGLMD", ""), Category::Icon);
        assert_eq!(cat("NRW T WIRE BLT MD", "33263546"), Category::Icon);
        assert_eq!(
            parse_tiffany_code("PT18Y CIRCLE PDT 16IN", "").material,
            "Platinum + 18k Yellow Gold"
        );
        assert_eq!(parse_tiffany_code("X", "69525857").collection, "Knot"); // duplicate key: last wins
        assert_eq!(
            parse_tiffany_code("TIFFANY 1837 RING", "").collection,
            "Tiffany 1837"
        );
    }

    #[test]
    fn sku_table_is_sorted() {
        assert!(SKU_TO_COLLECTION.windows(2).all(|w| w[0].0 < w[1].0));
    }
}
