#!/usr/bin/env bash
# Rust uygulamasını (rust/commission-app) WebAssembly'e derler → pkg/
#
# Kullanım : rust/build-web.sh
# Gerekli  : rustup target add wasm32-unknown-unknown
#            cargo install wasm-bindgen-cli --version 0.2.130   (Cargo.toml'daki sürümle aynı olmalı)
#
# Sonuç: pkg/commission_app.js (otomatik üretilen yükleyici) + pkg/commission_app_bg.wasm
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUST="$ROOT/rust"

cargo build --manifest-path "$RUST/Cargo.toml" --release -p commission-app --target wasm32-unknown-unknown
rm -rf "$ROOT/pkg"
wasm-bindgen --target web --no-typescript --out-dir "$ROOT/pkg" \
  "$RUST/target/wasm32-unknown-unknown/release/commission_app.wasm" 2> >(grep -v 'target_features' >&2)
rm -f "$ROOT/pkg/.gitignore"
echo "OK: pkg/ güncellendi — $(du -h "$ROOT/pkg/commission_app_bg.wasm" | cut -f1) wasm"
