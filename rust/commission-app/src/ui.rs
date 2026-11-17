//! The page: Rust connects the HTML (index.html) to the app logic.
//!
//! Port of all screen code of the old JavaScript app (v13.8). The HTML and
//! CSS did not change; ids, classes and texts are the same.

use std::cell::RefCell;
use std::cmp::Ordering;

use commission_core::Category;
use js_sys::{Array, Date, Object, Promise, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AbortController, Blob, BlobPropertyBag, Document, Element, Event, EventTarget,
    HtmlAnchorElement, HtmlElement, HtmlInputElement, HtmlSelectElement, KeyboardEvent, Response,
    Storage, Url, Window,
};

use crate::calc::{self, CATS};
use crate::fx::{self, FxRate};
use crate::import::{self, ImportError, Item};
use crate::js::{self, format_eur};
use crate::report::{self, Person};
use crate::state::{self, Rows, SaleRow, Settings};
use crate::xlsx::{self, SheetSpec, Theme};

// ─── App state ───────────────────────────────────────────────────────

struct App {
    rows: Rows,
    next_id: i64,
    settings: Settings,
    target_achieved: bool,
    parsed: Vec<Item>,
    import_errors: Vec<ImportError>,
    team: Vec<Person>,
    team_target: f64,
    fx_rate: Option<FxRate>,
    /// `true` = "My Sales" currency select, `false` = team select (old `fxContext`).
    fx_single: bool,
    save_timer: Option<i32>,
    toast_timer: Option<i32>,
    badge_timer: Option<i32>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` with the app state. Never call DOM code that fires events inside `f`.
fn with<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|a| f(a.borrow_mut().as_mut().expect("app started")))
}

// ─── Browser helpers ─────────────────────────────────────────────────

fn window() -> Window {
    web_sys::window().expect("window")
}

fn document() -> Document {
    window().document().expect("document")
}

fn by_id(id: &str) -> HtmlElement {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("#{id} missing"))
        .unchecked_into()
}

fn input(id: &str) -> HtmlInputElement {
    by_id(id).unchecked_into()
}

fn select_value(id: &str) -> String {
    document()
        .get_element_by_id(id)
        .map(|e| e.unchecked_into::<HtmlSelectElement>().value())
        .unwrap_or_else(|| "EUR".into())
}

fn set_text(id: &str, text: &str) {
    by_id(id).set_text_content(Some(text));
}

fn set_disabled(id: &str, disabled: bool) {
    by_id(id)
        .unchecked_into::<web_sys::HtmlButtonElement>()
        .set_disabled(disabled);
}

fn storage() -> Option<Storage> {
    window().local_storage().ok().flatten()
}

fn ls_get(key: &str) -> Option<String> {
    storage()?.get_item(key).ok().flatten()
}

fn ls_set(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, value);
    }
}

fn ls_remove(key: &str) {
    if let Some(s) = storage() {
        let _ = s.remove_item(key);
    }
}

fn on(target: &EventTarget, event: &str, f: impl FnMut(Event) + 'static) {
    let cb = Closure::<dyn FnMut(Event)>::new(f);
    let _ = target.add_event_listener_with_callback(event, cb.as_ref().unchecked_ref());
    cb.forget();
}

fn on_id(id: &str, event: &str, f: impl FnMut(Event) + 'static) {
    on(&by_id(id), event, f);
}

fn set_timeout(f: impl FnOnce() + 'static, ms: i32) -> i32 {
    let cb = Closure::once_into_js(f);
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms)
        .unwrap_or(0)
}

fn clear_timeout(id: Option<i32>) {
    if let Some(id) = id {
        window().clear_timeout_with_handle(id);
    }
}

async fn sleep(ms: i32) {
    let p = Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = JsFuture::from(p).await;
}

/// Local date `YYYY-MM-DD` (the old `todayISO()`).
fn today() -> String {
    let d = Date::new_0();
    format!(
        "{}-{:02}-{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date()
    )
}

fn generated_on() -> String {
    Date::new_0()
        .to_locale_string("en-US", &JsValue::UNDEFINED)
        .into()
}

fn locale_compare(a: &str, b: &str) -> Ordering {
    let r = js_sys::JsString::from(a).locale_compare(b, &Array::new(), &Object::new());
    r.cmp(&0)
}

fn closest(target: &EventTarget, selector: &str) -> Option<Element> {
    target
        .dyn_ref::<Element>()?
        .closest(selector)
        .ok()
        .flatten()
}

fn event_target(e: &Event) -> Option<EventTarget> {
    e.target()
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn err_text(e: &JsValue) -> String {
    if let Some(err) = e.dyn_ref::<js_sys::Error>() {
        return String::from(err.message());
    }
    e.as_string().unwrap_or_else(|| "unknown error".into())
}

// ─── Saving ──────────────────────────────────────────────────────────

fn flash_saved_badge() {
    let b = by_id("savedBadge");
    let _ = b.class_list().add_1("flash");
    let old = with(|a| a.badge_timer.take());
    clear_timeout(old);
    let t = set_timeout(
        || {
            let _ = by_id("savedBadge").class_list().remove_1("flash");
        },
        600,
    );
    with(|a| a.badge_timer = Some(t));
}

fn save_state() {
    let text = with(|a| state::save_rows(&a.rows, &String::from(Date::new_0().to_iso_string())));
    ls_set(state::DATA_KEY, &text);
    flash_saved_badge();
}

fn schedule_save() {
    let old = with(|a| a.save_timer.take());
    clear_timeout(old);
    let t = set_timeout(save_state, 250);
    with(|a| a.save_timer = Some(t));
    flash_saved_badge();
}

fn save_settings() {
    let text = with(|a| state::save_settings(&a.settings));
    ls_set(state::SETTINGS_KEY, &text);
}

fn collapsed_state() -> serde_json::Map<String, serde_json::Value> {
    ls_get(state::COLLAPSE_KEY)
        .and_then(|r| serde_json::from_str::<serde_json::Value>(&r).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn set_collapsed(cat: &str, collapsed: bool) {
    let mut s = collapsed_state();
    s.insert(cat.into(), serde_json::Value::Bool(collapsed));
    ls_set(
        state::COLLAPSE_KEY,
        &serde_json::Value::Object(s).to_string(),
    );
}

// ─── Main card ───────────────────────────────────────────────────────

fn recompute_target(a: &mut App) {
    a.target_achieved = calc::target_met(report::total_sales(&a.rows), a.settings.target_amount);
}

fn value_or_zero(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v }
}

/// `${row.value || ''}`
fn value_attr(v: f64) -> String {
    if v == 0.0 || v.is_nan() {
        String::new()
    } else {
        js::num_to_string(v)
    }
}

fn rate_line(bp: i64, n: usize) -> String {
    format!(
        "Rate: %{} • {} row{}",
        calc::pct_text(bp),
        n,
        if n == 1 { "" } else { "s" }
    )
}

/// The old `buildCategories()`.
fn build_categories() {
    let collapsed = collapsed_state();
    let html = with(|a| {
        recompute_target(a);
        let met = a.target_achieved;
        let t = today();
        let mut html = String::new();
        for (cat, list) in CATS.iter().zip(&a.rows) {
            let key = calc::key(*cat);
            let sum = list.iter().fold(0.0, |s, r| s + value_or_zero(r.value));
            let prim = list.iter().fold(0.0, |s, r| {
                s + calc::commission(*cat, value_or_zero(r.value), met)
            });
            let is_collapsed = collapsed.get(key).is_some_and(|v| {
                v.as_bool().unwrap_or(false) || v.as_f64().is_some_and(|f| f != 0.0)
            });
            let rows_html: String = list
                .iter()
                .enumerate()
                .map(|(idx, row)| {
                    let date = if row.date.is_empty() { t.clone() } else { row.date.clone() };
                    let day = js::day_name(&date);
                    format!(
                        r#"
            <div class="row-item" data-rowid="{id}" title="{title}">
              <div class="num">#{num}</div>
              <div class="date-wrap" title="{day} {fdate}">
                <input type="date" class="date-input" value="{date}" />
                <div class="day-name">{day}</div>
              </div>
              <input type="number" min="0" step="0.01" inputmode="decimal" value="{value}" placeholder="0" />
              <div class="row-prim">{prim}</div>
              <button class="del">✕</button>
            </div>"#,
                        id = row.id.map_or("undefined".into(), |i| i.to_string()),
                        title = escape_html(row.desc.as_deref().unwrap_or("")),
                        num = idx + 1,
                        fdate = js::format_date(&date),
                        value = value_attr(row.value),
                        prim = format_eur(calc::commission(*cat, value_or_zero(row.value), met)),
                    )
                })
                .collect();
            html.push_str(&format!(
                r#"<div class="cat-block {key}{toggled}{coll}">
      <div class="cat-head" data-toggle-cat="{key}" role="button" tabindex="0" aria-expanded="{expanded}">
        <div class="cat-icon">{emoji}</div>
        <div class="cat-title"><h3>{label}</h3><p>{rate}</p></div>
        <div class="cat-stats"><div class="sum">{sum}</div><div class="prim">Commission: {prim}</div></div>
        <div class="chevron">▼</div>
      </div>
      <div class="cat-body">
        <div class="rows" data-cat="{key}">
          {empty}
          {rows_html}
        </div>
        <button class="add-btn" data-add="{key}">+ Add Row</button>
      </div>
    </div>"#,
                toggled = if met { " toggled" } else { "" },
                coll = if is_collapsed { " collapsed" } else { "" },
                expanded = !is_collapsed,
                emoji = calc::emoji(*cat),
                label = cat.label(),
                rate = rate_line(calc::table_rate_bp(*cat, met), list.len()),
                sum = format_eur(sum),
                prim = format_eur(prim),
                empty = if list.is_empty() { r#"<div class="empty-hint">No rows yet. Add one below.</div>"# } else { "" },
            ));
        }
        html
    });
    by_id("catContainer").set_inner_html(&html);
}

/// The old `update()`.
fn update() {
    struct View {
        met: bool,
        target: f64,
        total_sales: f64,
        sums: [f64; 4],
        prims: [f64; 4],
        counts: [usize; 4],
        row_prims: Vec<(String, i64, f64)>,
    }
    let v = with(|a| {
        recompute_target(a);
        let met = a.target_achieved;
        let mut sums = [0.0; 4];
        let mut prims = [0.0; 4];
        let mut row_prims = Vec::new();
        for (i, (cat, list)) in CATS.iter().zip(&a.rows).enumerate() {
            for r in list {
                let v = value_or_zero(r.value);
                sums[i] += v;
                let p = calc::commission(*cat, v, met);
                prims[i] += p;
                if let Some(id) = r.id {
                    row_prims.push((calc::key(*cat).to_string(), id, p));
                }
            }
        }
        View {
            met,
            target: a.settings.target_amount,
            total_sales: report::total_sales(&a.rows),
            sums,
            prims,
            counts: [0, 1, 2, 3].map(|i| a.rows[i].len()),
            row_prims,
        }
    });
    let badge = by_id("targetBadge");
    badge.set_text_content(Some(if v.met {
        "Target: Met ✓"
    } else if v.target > 0.0 {
        "Target: Pending"
    } else {
        "Target: Not Set"
    }));
    let style = badge.style();
    let _ = style.set_property(
        "background",
        if v.met {
            "var(--tiffany-blue-bg)"
        } else {
            "var(--tiffany-gray-lighter)"
        },
    );
    let _ = style.set_property(
        "color",
        if v.met {
            "var(--tiffany-blue-dark)"
        } else {
            "var(--tiffany-gray)"
        },
    );
    let pct = if v.target > 0.0 {
        (v.total_sales / v.target * 100.0).min(100.0)
    } else {
        0.0
    };
    let fill = by_id("progressFill");
    let _ = fill
        .style()
        .set_property("width", &format!("{}%", js::num_to_string(pct)));
    let _ = fill.class_list().toggle_with_force("met", v.met);
    set_text(
        "progressText",
        &if v.target > 0.0 {
            format!("{} / {}", format_eur(v.total_sales), format_eur(v.target))
        } else {
            format!("{} / — (set a target)", format_eur(v.total_sales))
        },
    );
    let total = v.prims.iter().fold(0.0, |a, p| a + p);
    for (i, (id, label)) in [
        ("bSilver", "Silver"),
        ("bIcon", "Icon"),
        ("bOthers", "Others"),
        ("bWatches", "Watches"),
    ]
    .iter()
    .enumerate()
    {
        set_text(id, &format!("{label}: {}", format_eur(v.prims[i])));
    }
    set_text("totalPrim", &format_eur(total));

    let doc = document();
    for (cat, id, p) in &v.row_prims {
        let sel = format!(r#".rows[data-cat="{cat}"] .row-item[data-rowid="{id}"] .row-prim"#);
        if let Ok(Some(el)) = doc.query_selector(&sel) {
            el.set_text_content(Some(&format_eur(*p)));
        }
    }
    for (i, cat) in CATS.iter().enumerate() {
        let Ok(Some(block)) = doc.query_selector(&format!(".cat-block.{}", calc::key(*cat))) else {
            continue;
        };
        let _ = block.class_list().toggle_with_force("toggled", v.met);
        if let Ok(Some(e)) = block.query_selector(".cat-stats .sum") {
            e.set_text_content(Some(&format_eur(v.sums[i])));
        }
        if let Ok(Some(e)) = block.query_selector(".cat-stats .prim") {
            e.set_text_content(Some(&format!("Commission: {}", format_eur(v.prims[i]))));
        }
        if let Ok(Some(e)) = block.query_selector(".cat-title p") {
            e.set_text_content(Some(&rate_line(
                calc::table_rate_bp(*cat, v.met),
                v.counts[i],
            )));
        }
    }
    set_text("footerNote", &calc::footer_note());
}

fn show_toast(msg: &str, ok: bool) {
    let doc = document();
    let t: HtmlElement = match doc.query_selector(".export-toast").ok().flatten() {
        Some(e) => e.unchecked_into(),
        None => {
            let e: HtmlElement = doc.create_element("div").expect("div").unchecked_into();
            e.set_class_name("export-toast");
            if let Some(body) = doc.body() {
                let _ = body.append_child(&e);
            }
            e
        }
    };
    t.set_text_content(Some(msg));
    let _ = t.style().set_property(
        "background",
        if ok {
            "rgba(16, 185, 129, 0.95)"
        } else {
            "rgba(239, 68, 68, 0.95)"
        },
    );
    let t2 = t.clone();
    let cb = Closure::once_into_js(move || {
        let _ = t2.class_list().add_1("show");
    });
    let _ = window().request_animation_frame(cb.unchecked_ref());
    let old = with(|a| a.toast_timer.take());
    clear_timeout(old);
    let id = set_timeout(
        move || {
            let _ = t.class_list().remove_1("show");
        },
        2400,
    );
    with(|a| a.toast_timer = Some(id));
}

fn row_ref(target: &EventTarget) -> Option<(Category, i64, Element)> {
    let item = closest(target, ".row-item")?;
    let cat = calc::from_key(&item.closest(".rows").ok()??.get_attribute("data-cat")?)?;
    let id = js::parse_float(&item.get_attribute("data-rowid")?)? as i64;
    Some((cat, id, item))
}

fn wire_categories() {
    let container = by_id("catContainer");
    on(&container, "click", |e| {
        let Some(t) = event_target(&e) else { return };
        if let Some(head) = closest(&t, ".cat-head[data-toggle-cat]") {
            toggle_block(&head);
        } else if closest(&t, ".del").is_some() {
            if let Some((cat, id, _)) = row_ref(&t) {
                with(|a| a.rows[cat.index() as usize].retain(|r| r.id != Some(id)));
                build_categories();
                update();
                schedule_save();
            }
        } else if let Some(btn) = closest(&t, ".add-btn") {
            let Some(cat) = btn
                .get_attribute("data-add")
                .and_then(|k| calc::from_key(&k))
            else {
                return;
            };
            let t = today();
            with(|a| {
                let id = a.next_id;
                a.next_id += 1;
                a.rows[cat.index() as usize].push(SaleRow::new(id, 0.0, t));
            });
            build_categories();
            update();
            schedule_save();
            let sel = format!(r#".rows[data-cat="{}"] .row-item"#, calc::key(cat));
            if let Ok(list) = document().query_selector_all(&sel)
                && list.length() > 0
                && let Some(last) = list.item(list.length() - 1)
                && let Ok(Some(inp)) = last
                    .unchecked_into::<Element>()
                    .query_selector(r#"input[type="number"]"#)
            {
                let _ = inp.unchecked_into::<HtmlElement>().focus();
            }
        }
    });
    on(&container, "keydown", |e| {
        let Some(t) = event_target(&e) else { return };
        let Some(k) = e.dyn_ref::<KeyboardEvent>() else {
            return;
        };
        if (k.key() == "Enter" || k.key() == " ")
            && let Some(head) = closest(&t, ".cat-head[data-toggle-cat]")
            && t.dyn_ref::<Element>().is_some_and(|el| el == &head)
        {
            e.prevent_default();
            toggle_block(&head);
        }
    });
    on(&container, "input", |e| {
        let Some(t) = event_target(&e) else { return };
        let Some(inp) = t.dyn_ref::<HtmlInputElement>() else {
            return;
        };
        if inp.type_() != "number" {
            return;
        }
        let Some((cat, id, _)) = row_ref(&t) else {
            return;
        };
        let v = js::parse_float(&inp.value())
            .filter(|v| *v != 0.0 && !v.is_nan())
            .unwrap_or(0.0);
        let found = with(|a| {
            match a.rows[cat.index() as usize]
                .iter_mut()
                .find(|r| r.id == Some(id))
            {
                Some(r) => {
                    r.value = v;
                    true
                }
                None => false,
            }
        });
        if found {
            update();
            schedule_save();
        }
    });
    on(&container, "change", |e| {
        let Some(t) = event_target(&e) else { return };
        let Some(inp) = t.dyn_ref::<HtmlInputElement>() else {
            return;
        };
        if !inp.class_list().contains("date-input") {
            return;
        }
        let Some((cat, id, item)) = row_ref(&t) else {
            return;
        };
        let v = inp.value();
        let date = if v.is_empty() { today() } else { v };
        let found = with(|a| {
            match a.rows[cat.index() as usize]
                .iter_mut()
                .find(|r| r.id == Some(id))
            {
                Some(r) => {
                    r.date = date.clone();
                    true
                }
                None => false,
            }
        });
        if found {
            let dn = js::day_name(&date);
            if let Ok(Some(e)) = item.query_selector(".day-name") {
                e.set_text_content(Some(dn));
            }
            if let Ok(Some(w)) = item.query_selector(".date-wrap") {
                let _ = w.set_attribute("title", &format!("{dn} {}", js::format_date(&date)));
            }
            schedule_save();
        }
    });
}

fn toggle_block(head: &Element) {
    let Some(block) = head.closest(".cat-block").ok().flatten() else {
        return;
    };
    let cat = head.get_attribute("data-toggle-cat").unwrap_or_default();
    let now = !block.class_list().contains("collapsed");
    let _ = block.class_list().toggle_with_force("collapsed", now);
    let _ = head.set_attribute("aria-expanded", if now { "false" } else { "true" });
    set_collapsed(&cat, now);
}

// ─── Currency / FX ───────────────────────────────────────────────────

fn is_try() -> bool {
    let single = with(|a| a.fx_single);
    select_value(if single { "myCurrency" } else { "teamCurrency" }) == "TRY"
}

fn render_fx_note() {
    let el = by_id("fxNote");
    let fx = with(|a| a.fx_rate.clone());
    match fx::note_html(is_try(), fx.as_ref()) {
        None => {
            let _ = el.style().set_property("display", "none");
            el.set_inner_html("");
        }
        Some(h) => {
            let _ = el.style().set_property("display", "block");
            el.set_inner_html(&h);
        }
    }
}

struct FxErr {
    no_data: bool,
    msg: String,
}

async fn fetch_with_timeout(url: &str, ms: i32) -> Result<Response, String> {
    let ctrl = AbortController::new().map_err(|e| err_text(&e))?;
    let init = web_sys::RequestInit::new();
    init.set_signal(Some(&ctrl.signal()));
    let c2 = ctrl.clone();
    let timer = set_timeout(move || c2.abort(), ms);
    let res = JsFuture::from(window().fetch_with_str_and_init(url, &init)).await;
    clear_timeout(Some(timer));
    res.map(|r| r.unchecked_into()).map_err(|e| err_text(&e))
}

async fn response_text(res: &Response) -> Result<String, String> {
    let p = res.text().map_err(|e| err_text(&e))?;
    JsFuture::from(p)
        .await
        .map(|v| v.as_string().unwrap_or_default())
        .map_err(|e| err_text(&e))
}

/// The old `fetchTcmbRateForDate(ds)`.
async fn fetch_tcmb_rate_for_date(ds: &str) -> Result<f64, FxErr> {
    let mut net_err = String::new();
    for attempt in 0..3 {
        let url = format!("{}/tcmb?date={ds}", fx::TCMB_WORKER);
        match fetch_with_timeout(&url, 12_000).await {
            Ok(res) => {
                let j = match response_text(&res).await {
                    Ok(t) => serde_json::from_str::<serde_json::Value>(&t)
                        .ok()
                        .filter(|v| !v.is_null()),
                    Err(_) => None,
                };
                if let Some(j) = j {
                    let rate = j
                        .get("rate")
                        .and_then(|r| r.as_f64().or_else(|| r.as_str().and_then(js::parse_float)));
                    if let Some(r) = rate.filter(|r| *r != 0.0 && !r.is_nan()) {
                        return Ok(r);
                    }
                    let msg = j
                        .get("error")
                        .and_then(|e| e.as_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("no data");
                    return Err(FxErr {
                        no_data: true,
                        msg: msg.into(),
                    });
                }
                net_err = format!("boş cevap (HTTP {})", res.status());
            }
            Err(e) => net_err = e,
        }
        sleep(350 * (attempt + 1)).await;
    }
    let target = fx::tcmb_xml_url(ds);
    let mut last_err: Option<String> = None;
    for (url, wrapped) in fx::proxies(&target) {
        let result: Result<f64, FxErr> = async {
            let res = fetch_with_timeout(&url, 12_000).await.map_err(|m| FxErr {
                no_data: false,
                msg: m,
            })?;
            if res.status() == 404 {
                return Err(FxErr {
                    no_data: true,
                    msg: "TCMB o gün yayın yapmamış (404)".into(),
                });
            }
            if !res.ok() {
                return Err(FxErr {
                    no_data: false,
                    msg: format!("HTTP {}", res.status()),
                });
            }
            let mut text = response_text(&res).await.map_err(|m| FxErr {
                no_data: false,
                msg: m,
            })?;
            if wrapped {
                text = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v.get("contents").and_then(|c| c.as_str()).map(String::from))
                    .ok_or(FxErr {
                        no_data: false,
                        msg: "bad json".into(),
                    })?;
            }
            fx::parse_tcmb_xml(&text).map_err(|m| FxErr {
                no_data: false,
                msg: m,
            })
        }
        .await;
        match result {
            Ok(v) => return Ok(v),
            Err(e) if e.no_data => return Err(e),
            Err(e) => last_err = Some(e.msg),
        }
    }
    let why = last_err.unwrap_or(net_err);
    Err(FxErr {
        no_data: false,
        msg: format!(
            "TCMB kur kaynağına ulaşılamadı: {}",
            if why.is_empty() {
                "bilinmeyen hata".into()
            } else {
                why
            }
        ),
    })
}

/// The old `fetchRateOnOrBefore(ds)`: (rate, day of the rate).
async fn fetch_rate_on_or_before(ds: &str) -> Result<(f64, String), String> {
    let mut day = ds.to_string();
    for _ in 0..fx::FX_MAX_BACK {
        match fetch_tcmb_rate_for_date(&day).await {
            Ok(r) => return Ok((r, day)),
            Err(e) if e.no_data => day = fx::previous_day(&day),
            Err(e) => {
                return Err(format!(
                    "TCMB kuru alınamadı — internet bağlantınızı kontrol edin. ({})",
                    e.msg
                ));
            }
        }
    }
    Err(format!(
        "TCMB'de {} gün geriye kadar yayımlanmış kur bulunamadı ({ds})",
        fx::FX_MAX_BACK
    ))
}

/// The old `loadFxRatesForDates(isoDates, onStatus)`.
async fn load_fx_rate(dates: Vec<String>, on_status: Option<fn(&str)>) -> Result<(), String> {
    let t = today();
    let month = fx::report_month(dates.iter().map(String::as_str), &t);
    let (ds, is_current) = fx::target_for_month(&month, &t);
    let cached = with(|a| {
        a.fx_rate
            .as_ref()
            .is_some_and(|f| f.target == ds && f.rate > 0.0)
    });
    if cached {
        return Ok(());
    }
    if let Some(cb) = on_status {
        cb(&format!(
            "TCMB kuru alınıyor (rapor ayı {month}{})…",
            if is_current { " — ay sürüyor" } else { "" }
        ));
    }
    let (rate, date) = fetch_rate_on_or_before(&ds).await?;
    with(|a| {
        a.fx_rate = Some(FxRate {
            rate,
            date: date.clone(),
            target: ds,
            month: month.clone(),
            is_current,
        })
    });
    if let Some(cb) = on_status {
        cb(&format!(
            "✓ TCMB kuru {month}: {date} = {}",
            js::num_to_string(rate)
        ));
    }
    render_fx_note();
    Ok(())
}

// ─── Downloads ───────────────────────────────────────────────────────

fn download(bytes: &[u8], name: &str) -> Result<(), String> {
    let arr = Uint8Array::from(bytes);
    let parts = Array::of1(&arr);
    let opts = BlobPropertyBag::new();
    opts.set_type("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
    let blob =
        Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(|e| err_text(&e))?;
    let url = Url::create_object_url_with_blob(&blob).map_err(|e| err_text(&e))?;
    let a: HtmlAnchorElement = document()
        .create_element("a")
        .map_err(|e| err_text(&e))?
        .unchecked_into();
    a.set_href(&url);
    a.set_download(name);
    let body = document().body().ok_or("no body")?;
    let _ = body.append_child(&a);
    a.click();
    let _ = body.remove_child(&a);
    set_timeout(
        move || {
            let _ = Url::revoke_object_url(&url);
        },
        10_000,
    );
    Ok(())
}

// ─── Excel (My Sales) ────────────────────────────────────────────────

fn has_any_sales() -> bool {
    with(|a| {
        a.rows
            .iter()
            .flatten()
            .any(|r| value_or_zero(r.value) > 0.0)
    })
}

async fn export_to_excel() {
    if !has_any_sales() {
        show_toast(
            "This report is for YOUR sales only — import a file first, or use 👥 Team Report",
            false,
        );
        return;
    }
    with(|a| a.fx_single = true);
    let btn = by_id("exportExcelBtn");
    let label = btn.text_content().unwrap_or_default();
    let result: Result<(), String> = async {
        if is_try() {
            set_disabled("exportExcelBtn", true);
            btn.set_text_content(Some("⏳ Fetching rates…"));
            let dates = with(|a| {
                recompute_target(a);
                report::flatten_rows(&a.rows, a.target_achieved, &today())
                    .into_iter()
                    .map(|l| l.date)
                    .collect()
            });
            load_fx_rate(dates, None).await?;
        }
        let fx_on = is_try();
        let (bytes, name) = with(|a| -> Result<_, String> {
            recompute_target(a);
            let t = today();
            let fx_rate = if fx_on { a.fx_rate.as_ref() } else { None };
            let data = report::build_my_report(&report::MyReport {
                rows: &a.rows,
                target_amount: a.settings.target_amount,
                salesperson: &a.settings.salesperson_name,
                generated_on: &generated_on(),
                today: &t,
                fx: fx_rate,
            });
            let widths = report::col_widths("single", fx_on);
            let bytes = xlsx::workbook_bytes(&[SheetSpec {
                name: "Commission Report",
                data: &data,
                widths: &widths,
                theme: Theme::Single,
            }])?;
            let lines = report::flatten_rows(&a.rows, a.target_achieved, &t);
            Ok((
                bytes,
                report::my_report_file_name(&a.settings.salesperson_name, &lines, &t),
            ))
        })?;
        download(&bytes, &name)?;
        let short = with(|a| fx::summary_short(a.fx_rate.as_ref()));
        show_toast(
            &if fx_on {
                format!("✓ Excel downloaded (TRY @ {short})")
            } else {
                "✓ Excel downloaded".into()
            },
            true,
        );
        Ok(())
    }
    .await;
    if let Err(e) = result {
        show_toast(&format!("Export failed: {e}"), false);
    }
    set_disabled("exportExcelBtn", false);
    btn.set_text_content(Some(&label));
}

// ─── Import Excel (My Sales) ─────────────────────────────────────────

fn set_xlsx_status(kind: &str, text: &str) {
    by_id("xlsxStatus").set_class_name(&format!("llm-status show {kind}"));
    set_text("xlsxStatusText", text);
}

fn open_xlsx_modal() {
    with(|a| {
        a.parsed.clear();
        a.import_errors.clear();
    });
    let _ = by_id("xlsxModal").class_list().add_1("show");
    by_id("parseResults").set_inner_html("");
    input("xlsxFileInput").set_value("");
    set_disabled("xlsxAddBtn", true);
    set_xlsx_status("processing", "Select Excel file (.xlsx)");
}

fn close_modal(id: &str) {
    let _ = by_id(id).class_list().remove_1("show");
}

fn is_excel_name(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".xlsx") || l.ends_with(".xls")
}

async fn file_bytes(file: &web_sys::File) -> Result<Vec<u8>, String> {
    let buf = JsFuture::from(file.array_buffer())
        .await
        .map_err(|e| err_text(&e))?;
    Ok(Uint8Array::new(&buf).to_vec())
}

fn handle_xlsx_file(file: web_sys::File) {
    if !is_excel_name(&file.name()) {
        set_xlsx_status("error", "Only .xlsx / .xls files are supported");
        return;
    }
    set_xlsx_status("processing", &format!("{} okunuyor…", file.name()));
    spawn_local(async move {
        if let Err(e) = import_excel_local(&file).await {
            set_xlsx_status("error", &format!("Hata: {e}"));
        }
    });
}

async fn import_excel_local(file: &web_sys::File) -> Result<(), String> {
    let bytes = file_bytes(file).await?;
    let wb = import::read_workbook(&bytes)?;
    let r = import::import_single(&wb)?;
    if let Some(name) = r.salesperson.clone() {
        with(|a| a.settings.salesperson_name = name);
        save_settings();
    }
    let t = today();
    let mut msg = format!("✓ {} products read (local parse)", r.items.len());
    if r.header_row > 0 {
        msg.push_str(&format!(" — header detected at row {}", r.header_row + 1));
    }
    if !r.errors.is_empty() {
        msg.push_str(&format!(" — {} rows skipped", r.errors.len()));
    }
    let fut = import::future_date_count(r.items.iter().map(|i| i.date.as_str()), &t);
    if fut > 0 {
        msg.push_str(&format!(
            " — ⚠ {fut} rows have FUTURE dates (check the year!)"
        ));
    }
    set_xlsx_status(
        if !r.errors.is_empty() || fut > 0 {
            "processing"
        } else {
            "success"
        },
        &msg,
    );
    if r.items.is_empty() {
        return Err("No valid sales found".into());
    }
    with(|a| {
        a.parsed = r.items;
        a.import_errors = r.errors;
    });
    render_parse_table();
    set_disabled("xlsxAddBtn", false);
    Ok(())
}

/// The old `renderParseTable()` (+ the skipped-rows box, which the old code hid by mistake).
fn render_parse_table() {
    let html = with(|a| {
        if a.parsed.is_empty() {
            return String::new();
        }
        let mut by_cat = [0usize; 4];
        for it in &a.parsed {
            by_cat[it.category.index() as usize] += 1;
        }
        let pills: String = CATS
            .iter()
            .map(|c| {
                format!(
                    r#"<div class="pill">{} <strong>{}</strong> {}</div>"#,
                    calc::emoji(*c),
                    by_cat[c.index() as usize],
                    c.label()
                )
            })
            .collect();
        let mut out = String::new();
        if !a.import_errors.is_empty() {
            let shown: Vec<String> = a
                .import_errors
                .iter()
                .take(5)
                .map(|e| format!("Row {}: {}", e.row, escape_html(&e.reason)))
                .collect();
            let more = if a.import_errors.len() > 5 {
                format!("<br>... and {} more rows", a.import_errors.len() - 5)
            } else {
                String::new()
            };
            out.push_str(&format!(
                r#"<div style="margin-top: 8px; padding: 10px; background: rgba(245, 158, 11, 0.1); border: 1px solid rgba(245, 158, 11, 0.3); border-radius: 6px; font-size: 0.78rem; color: #92400e;"><strong>⚠ Skipped rows:</strong> {}{more}</div>"#,
                shown.join("<br>")
            ));
        }
        out.push_str(&format!(r#"<div class="summary-bar"><div class="pill">Total: <strong>{}</strong> products</div>{pills}</div>"#, a.parsed.len()));
        let rows: String = a
            .parsed
            .iter()
            .enumerate()
            .map(|(idx, it)| {
                let coll_mat = [it.collection.as_str(), it.material.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" / ");
                let opts: String = CATS
                    .iter()
                    .map(|c| {
                        format!(
                            r#"<option value="{}" {}>{} {}</option>"#,
                            calc::key(*c),
                            if *c == it.category { "selected" } else { "" },
                            calc::emoji(*c),
                            c.label()
                        )
                    })
                    .collect();
                format!(
                    r#"<tr data-idx="{idx}">
      <td>{n}</td>
      <td><input type="text" data-field="sku" value="{sku}" /></td>
      <td><input type="text" data-field="code" value="{code}" /></td>
      <td><input type="text" data-field="date" value="{date}" /></td>
      <td><input type="number" data-field="amount" value="{amount}" step="0.01" /></td>
      <td title="{cm}" style="font-size: 0.75rem; color: #94a3b8; max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{cm_text}</td>
      <td><select data-field="category">{opts}</select></td>
      <td><button class="row-remove" data-remove="{idx}">×</button></td>
    </tr>"#,
                    n = idx + 1,
                    sku = escape_html(&it.sku),
                    code = escape_html(&it.code),
                    date = escape_html(&it.date),
                    amount = js::num_to_string(it.amount),
                    cm = escape_html(&coll_mat),
                    cm_text = if coll_mat.is_empty() { "—".into() } else { escape_html(&coll_mat) },
                )
            })
            .collect();
        out.push_str(&format!(
            r#"<table class="parse-table"><thead><tr><th>#</th><th>SKU</th><th>Code</th><th>Date</th><th>Amount (€)</th><th>Collection / Material</th><th>Category</th><th></th></tr></thead><tbody>{rows}</tbody></table>"#
        ));
        out
    });
    by_id("parseResults").set_inner_html(&html);
}

fn wire_parse_table() {
    let box_ = by_id("parseResults");
    on(&box_, "change", |e| {
        let Some(t) = event_target(&e) else { return };
        let Some(el) = t.dyn_ref::<Element>() else {
            return;
        };
        let Some(field) = el.get_attribute("data-field") else {
            return;
        };
        let Some(idx) = closest(&t, "tr")
            .and_then(|tr| tr.get_attribute("data-idx"))
            .and_then(|i| i.parse::<usize>().ok())
        else {
            return;
        };
        let value = if let Some(i) = t.dyn_ref::<HtmlInputElement>() {
            i.value()
        } else if let Some(s) = t.dyn_ref::<HtmlSelectElement>() {
            s.value()
        } else {
            return;
        };
        with(|a| {
            let Some(it) = a.parsed.get_mut(idx) else {
                return;
            };
            match field.as_str() {
                "amount" => {
                    it.amount = js::parse_float(&value)
                        .filter(|v| *v != 0.0 && !v.is_nan())
                        .unwrap_or(0.0)
                }
                "category" => {
                    if let Some(c) = calc::from_key(&value) {
                        it.category = c;
                    }
                }
                "sku" => it.sku = value,
                "code" => it.code = value,
                "date" => it.date = value,
                _ => {}
            }
        });
    });
    on(&box_, "click", |e| {
        let Some(t) = event_target(&e) else { return };
        let Some(btn) = closest(&t, "[data-remove]") else {
            return;
        };
        let Some(idx) = btn
            .get_attribute("data-remove")
            .and_then(|i| i.parse::<usize>().ok())
        else {
            return;
        };
        with(|a| {
            if idx < a.parsed.len() {
                a.parsed.remove(idx);
            }
        });
        render_parse_table();
    });
}

fn add_parsed_to_sales() {
    let t = today();
    let added = with(|a| {
        let items = a.parsed.clone();
        for it in &items {
            let id = a.next_id;
            a.next_id += 1;
            let mut row = SaleRow::new(
                id,
                if it.amount.is_nan() { 0.0 } else { it.amount },
                if it.date.is_empty() {
                    t.clone()
                } else {
                    it.date.clone()
                },
            );
            row.desc = Some(it.code.clone());
            row.sku = Some(it.sku.clone());
            a.rows[it.category.index() as usize].push(row);
        }
        items.len()
    });
    if added == 0 {
        return;
    }
    build_categories();
    update();
    schedule_save();
    show_toast(&format!("✓ {added} products added"), true);
    close_modal("xlsxModal");
}

// ─── Team report ─────────────────────────────────────────────────────

fn team_status(kind: &str, text: &str) {
    by_id("teamStatus").set_class_name(&format!("llm-status show {kind}"));
    set_text("teamStatusText", text);
}

fn team_status_processing(text: &str) {
    team_status("processing", text);
}

fn team_target_from_input() -> f64 {
    js::parse_float(&input("teamTargetInput").value())
        .filter(|v| !v.is_nan())
        .unwrap_or(0.0)
        .max(0.0)
}

fn render_team_preview() {
    let html = with(|a| {
        if a.team.is_empty() {
            return String::new();
        }
        let grand_sales = a.team.iter().fold(0.0, |s, p| s + p.total_sales);
        let grand_prim = a.team.iter().fold(0.0, |s, p| s + p.total_prim);
        let rows: String = a
            .team
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "<tr>\n    <td>{}</td>\n    <td style=\"text-align:left;\">{}</td>\n    <td>{}</td>\n    <td>{}</td>\n    <td>{}</td>\n    <td>{}</td>\n  </tr>",
                    i + 1,
                    escape_html(&p.name),
                    p.items.len(),
                    format_eur(p.total_sales),
                    format_eur(p.total_prim),
                    if p.achieved { "✓" } else { "✗" }
                )
            })
            .collect();
        format!(
            r#"<div class="summary-bar">
    <div class="pill">People: <strong>{}</strong></div>
    <div class="pill">Sales: <strong>{}</strong></div>
    <div class="pill">Commission: <strong>{}</strong></div>
  </div><table class="parse-table">
    <thead><tr><th>#</th><th>Salesperson</th><th>Sales</th><th>Total (€)</th><th>Commission (€)</th><th>Target</th></tr></thead>
    <tbody>{rows}</tbody></table>"#,
            a.team.len(),
            format_eur(grand_sales),
            format_eur(grand_prim)
        )
    });
    by_id("teamResults").set_inner_html(&html);
}

fn open_team_modal() {
    let _ = by_id("teamModal").class_list().add_1("show");
    input("teamFileInput").set_value("");
    let ti = input("teamTargetInput");
    if ti.value().is_empty() {
        let t = with(|a| a.settings.target_amount);
        ti.set_value(&value_attr(t));
    }
    let n = with(|a| a.team.len());
    if n > 0 {
        render_team_preview();
        set_disabled("teamExportBtn", false);
        set_disabled("teamExportSepBtn", false);
        team_status(
            "success",
            &format!("✓ {n} salespeople loaded — export again or upload a new file"),
        );
    } else {
        by_id("teamResults").set_inner_html("");
        set_disabled("teamExportBtn", true);
        set_disabled("teamExportSepBtn", true);
        team_status("processing", "Select bulk sales file (.xlsx)");
    }
}

fn handle_team_file(file: web_sys::File) {
    if !is_excel_name(&file.name()) {
        team_status("error", "Only .xlsx / .xls files are supported");
        return;
    }
    team_status("processing", &format!("{} okunuyor…", file.name()));
    spawn_local(async move {
        if let Err(e) = parse_team_workbook(&file).await {
            team_status("error", &format!("Hata: {e}"));
        }
    });
}

async fn parse_team_workbook(file: &web_sys::File) -> Result<(), String> {
    let bytes = file_bytes(file).await?;
    let wb = import::read_workbook(&bytes)?;
    let groups = import::import_team(&wb);
    let target = team_target_from_input();
    let team: Vec<Person> = groups
        .into_iter()
        .map(|(name, items)| report::person_stats(name, items, target))
        .collect();
    if team.is_empty() {
        return Err("No valid sales found in the file".into());
    }
    let t = today();
    let total_rows: usize = team.iter().map(|p| p.items.len()).sum();
    let fut = import::future_date_count(
        team.iter()
            .flat_map(|p| p.items.iter().map(|i| i.date.as_str())),
        &t,
    );
    let n = team.len();
    with(|a| {
        a.team = team;
        a.team_target = target;
    });
    team_status(
        if fut > 0 { "processing" } else { "success" },
        &format!(
            "✓ {n} salespeople, {total_rows} sales read{}",
            if fut > 0 {
                format!(" — ⚠ {fut} rows have FUTURE dates (check the year!)")
            } else {
                String::new()
            }
        ),
    );
    render_team_preview();
    set_disabled("teamExportBtn", false);
    set_disabled("teamExportSepBtn", false);
    Ok(())
}

async fn ensure_fx_rates() -> Result<(), String> {
    if !is_try() {
        return Ok(());
    }
    let dates: Vec<String> = with(|a| {
        a.team
            .iter()
            .flat_map(|p| p.flat.iter().map(|x| x.date.clone()))
            .collect()
    });
    load_fx_rate(dates, Some(team_status_processing)).await
}

fn cur_suffix() -> &'static str {
    if is_try() { "-TRY" } else { "" }
}

async fn export_team_excel() {
    if with(|a| a.team.is_empty()) {
        return;
    }
    with(|a| a.fx_single = false);
    let result: Result<usize, String> = async {
        ensure_fx_rates().await?;
        let fx_on = is_try();
        let suffix = cur_suffix();
        let (bytes, n) = with(|a| -> Result<_, String> {
            let fx_rate = if fx_on { a.fx_rate.as_ref() } else { None };
            let gen_on = generated_on();
            let mut used = Vec::new();
            let summary = report::build_team_summary(&a.team, a.team_target, &gen_on, fx_rate);
            let all = report::build_all_sales(&a.team, fx_rate, &locale_compare);
            let people: Vec<_> = a
                .team
                .iter()
                .map(|p| report::build_person_sheet(p, a.team_target, &gen_on, fx_rate))
                .collect();
            let names: Vec<String> = std::iter::once(report::safe_sheet_name("Summary", &mut used))
                .chain(std::iter::once(report::safe_sheet_name(
                    "All Sales",
                    &mut used,
                )))
                .chain(
                    a.team
                        .iter()
                        .map(|p| report::safe_sheet_name(&p.name, &mut used)),
                )
                .collect();
            let (ws, wa, wp) = (
                report::col_widths("summary", fx_on),
                report::col_widths("all", fx_on),
                report::col_widths("person", fx_on),
            );
            let mut specs = vec![
                SheetSpec {
                    name: &names[0],
                    data: &summary,
                    widths: &ws,
                    theme: Theme::Team,
                },
                SheetSpec {
                    name: &names[1],
                    data: &all,
                    widths: &wa,
                    theme: Theme::Team,
                },
            ];
            for (i, d) in people.iter().enumerate() {
                specs.push(SheetSpec {
                    name: &names[i + 2],
                    data: d,
                    widths: &wp,
                    theme: Theme::Team,
                });
            }
            Ok((xlsx::workbook_bytes(&specs)?, a.team.len()))
        })?;
        download(
            &bytes,
            &format!("team-commission-report{suffix}-{}.xlsx", today()),
        )?;
        Ok(n)
    }
    .await;
    match result {
        Ok(n) => {
            show_toast(&format!("✓ {n} reports exported"), true);
            let short = with(|a| fx::summary_short(a.fx_rate.as_ref()));
            team_status(
                "success",
                &format!(
                    "✓ Exported{}. You can export again or close this window.",
                    if is_try() {
                        format!(" (TRY @ {short})")
                    } else {
                        String::new()
                    }
                ),
            );
        }
        Err(e) => {
            team_status("error", &e);
            show_toast(&format!("Export failed: {e}"), false);
        }
    }
}

async fn export_team_separate() {
    if with(|a| a.team.is_empty()) {
        return;
    }
    with(|a| a.fx_single = false);
    if let Err(e) = ensure_fx_rates().await {
        team_status("error", &e);
        show_toast(&format!("Export failed: {e}"), false);
        return;
    }
    let fx_on = is_try();
    let suffix = cur_suffix();
    let total = with(|a| a.team.len());
    for i in 0..total {
        if i > 0 {
            sleep(400).await;
        }
        let file = with(|a| -> Result<_, String> {
            let p = &a.team[i];
            let fx_rate = if fx_on { a.fx_rate.as_ref() } else { None };
            let d = report::build_person_sheet(p, a.team_target, &generated_on(), fx_rate);
            let name = report::safe_sheet_name(&p.name, &mut Vec::new());
            let widths = report::col_widths("person", fx_on);
            let bytes = xlsx::workbook_bytes(&[SheetSpec {
                name: &name,
                data: &d,
                widths: &widths,
                theme: Theme::Team,
            }])?;
            Ok((
                bytes,
                format!(
                    "commission-{}{suffix}-{}.xlsx",
                    report::safe_file_part(&p.name),
                    today()
                ),
            ))
        });
        match file.and_then(|(bytes, name)| download(&bytes, &name)) {
            Ok(()) => {
                let done = i + 1;
                team_status("processing", &format!("Downloading… {done}/{total}"));
                if done == total {
                    team_status("success", &format!("✓ {done} separate files downloaded"));
                    show_toast(&format!("✓ {done} files downloaded"), true);
                }
            }
            Err(e) => team_status("error", &format!("Export failed: {e}")),
        }
    }
}

// ─── Drop zones ──────────────────────────────────────────────────────

fn wire_drop_zone(zone_id: &str, input_id: &'static str, handle: fn(web_sys::File)) {
    let zone = by_id(zone_id);
    on(&zone, "click", move |e| {
        // the file input sits inside the zone: its own click must not open the dialog twice
        if e.target()
            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
            .is_some()
        {
            return;
        }
        input(input_id).click();
    });
    on_id(input_id, "change", move |_| {
        if let Some(f) = input(input_id).files().and_then(|l| l.get(0)) {
            handle(f);
        }
    });
    for ev in ["dragenter", "dragover"] {
        let z = zone.clone();
        on(&zone, ev, move |e| {
            e.prevent_default();
            let _ = z.class_list().add_1("drag-over");
        });
    }
    for ev in ["dragleave", "drop"] {
        let z = zone.clone();
        on(&zone, ev, move |e| {
            e.prevent_default();
            let _ = z.class_list().remove_1("drag-over");
        });
    }
    on(&zone, "drop", move |e| {
        e.prevent_default();
        let file = e
            .dyn_ref::<web_sys::DragEvent>()
            .and_then(|d| d.data_transfer())
            .and_then(|dt| dt.files())
            .and_then(|l| l.get(0));
        if let Some(f) = file {
            handle(f);
        }
    });
}

// ─── Start ───────────────────────────────────────────────────────────

fn clear_state() {
    ls_remove(state::DATA_KEY);
    let t = today();
    with(|a| {
        a.rows = state::default_rows(&t);
        a.next_id = 5;
    });
    build_categories();
    update();
    show_toast("✓ Data reset", true);
}

#[wasm_bindgen(start)]
pub fn start() {
    // A Rust error (panic) is written to the browser console before the app stops.
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&format!("[CommissionApp] Rust panic: {info}").into());
    }));
    let t = today();
    let (rows, next_id) = state::load_rows(ls_get(state::DATA_KEY).as_deref(), &t);
    let settings = state::load_settings(ls_get(state::SETTINGS_KEY).as_deref());
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            rows,
            next_id,
            settings,
            target_achieved: false,
            parsed: Vec::new(),
            import_errors: Vec::new(),
            team: Vec::new(),
            team_target: 0.0,
            fx_rate: None,
            fx_single: true,
            save_timer: None,
            toast_timer: None,
            badge_timer: None,
        })
    });

    // Settings
    let target = with(|a| a.settings.target_amount);
    input("targetAmountInput").set_value(&value_attr(target));
    on_id("targetAmountInput", "input", |_| {
        let v = js::parse_float(&input("targetAmountInput").value())
            .filter(|v| !v.is_nan())
            .unwrap_or(0.0)
            .max(0.0);
        with(|a| a.settings.target_amount = v);
        save_settings();
        update();
    });

    wire_categories();
    wire_parse_table();

    // Import modal
    on_id("xlsxBtn", "click", |_| open_xlsx_modal());
    on_id("xlsxCloseBtn", "click", |_| close_modal("xlsxModal"));
    on_id("xlsxCancelBtn", "click", |_| close_modal("xlsxModal"));
    on_id("xlsxModal", "click", |e| {
        if e.target()
            .and_then(|t| t.dyn_into::<Element>().ok())
            .is_some_and(|el| el.id() == "xlsxModal")
        {
            close_modal("xlsxModal");
        }
    });
    wire_drop_zone("dropZone", "xlsxFileInput", handle_xlsx_file);
    on_id("xlsxAddBtn", "click", |_| add_parsed_to_sales());

    // Team modal
    on_id("teamBtn", "click", |_| open_team_modal());
    on_id("teamCloseBtn", "click", |_| close_modal("teamModal"));
    on_id("teamCancelBtn", "click", |_| close_modal("teamModal"));
    on_id("teamModal", "click", |e| {
        if e.target()
            .and_then(|t| t.dyn_into::<Element>().ok())
            .is_some_and(|el| el.id() == "teamModal")
        {
            close_modal("teamModal");
        }
    });
    on_id("teamExportBtn", "click", |_| {
        spawn_local(export_team_excel())
    });
    on_id("teamExportSepBtn", "click", |_| {
        spawn_local(export_team_separate())
    });
    wire_drop_zone("teamDropZone", "teamFileInput", handle_team_file);
    on_id("teamTargetInput", "input", |_| {
        let target = team_target_from_input();
        let has = with(|a| {
            a.team_target = target;
            if a.team.is_empty() {
                return false;
            }
            let team = std::mem::take(&mut a.team);
            a.team = team
                .into_iter()
                .map(|p| report::person_stats(p.name, p.items, target))
                .collect();
            true
        });
        if has {
            render_team_preview();
        }
    });

    // Currency selects
    on_id("myCurrency", "change", |_| {
        with(|a| a.fx_single = true);
        render_fx_note();
    });
    on_id("teamCurrency", "change", |_| {
        with(|a| a.fx_single = false);
        render_fx_note();
    });

    // Buttons
    on_id(
        "exportExcelBtn",
        "click",
        |_| spawn_local(export_to_excel()),
    );
    on_id("resetBtn", "click", |_| {
        let n = with(|a| a.rows.iter().map(Vec::len).sum::<usize>());
        if window()
            .confirm_with_message(&format!("Delete all data? ({n} rows will be lost)"))
            .unwrap_or(false)
        {
            clear_state();
        }
    });

    build_categories();
    update();
    render_fx_note();
    web_sys::console::info_1(&"[CommissionApp] Rust/WASM app started".into());
}
