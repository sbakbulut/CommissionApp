//! CommissionApp — the whole web app in Rust.
//!
//! Pure parts (tested with `cargo test`): code parser, file reading, maths,
//! reports, FX logic, saved data. The `ui` module (WebAssembly only) connects
//! them to the page.

pub mod calc;
pub mod fx;
pub mod import;
pub mod js;
pub mod report;
pub mod state;
pub mod tables;
pub mod tiffany;
pub mod xlsx;

#[cfg(target_arch = "wasm32")]
mod ui;
