#!/usr/bin/env bash
# Rust hesap çekirdeğini WebAssembly'e derler ve index.html içine gömer.
#
# Kullanım: rust/build-wasm.sh
# Gerekli : rustup target add wasm32-unknown-unknown
#
# Sonuç: index.html içindeki `const COMMISSION_WASM_B64 = '...';` satırı güncellenir.
# Uygulama tek dosya kalır ve çevrimdışı çalışır (ayrı .wasm dosyası yok).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RUST="$ROOT/rust"
WASM="$RUST/target/wasm32-unknown-unknown/release/commission_wasm.wasm"

cargo build --manifest-path "$RUST/Cargo.toml" --release -p commission-wasm --target wasm32-unknown-unknown

python3 - "$WASM" "$ROOT/index.html" <<'PY'
import base64, re, sys
wasm_path, html_path = sys.argv[1], sys.argv[2]
b64 = base64.b64encode(open(wasm_path, 'rb').read()).decode()
html = open(html_path, encoding='utf-8').read()
pattern = r"const COMMISSION_WASM_B64 = '[A-Za-z0-9+/=]*'; // @generated rust/build-wasm.sh"
if len(re.findall(pattern, html)) != 1:
    sys.exit('index.html içinde COMMISSION_WASM_B64 satırı tam 1 kez bulunmalı')
html = re.sub(pattern, f"const COMMISSION_WASM_B64 = '{b64}'; // @generated rust/build-wasm.sh", html)
open(html_path, 'w', encoding='utf-8').write(html)
print(f'index.html güncellendi: WASM {len(b64) * 3 // 4} bayt')
PY
