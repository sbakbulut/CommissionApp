#!/usr/bin/env bash
# publish.sh — Rust uygulamasını derler, denetler, commit'ler ve GitHub'a push eder.
#
# v14.0'dan itibaren uygulamanın tamamı Rust ile yazılır (rust/commission-app).
# Telefondaki "/sdcard/.../commission calculator.html" dosyası ARTIK KULLANILMAZ.
#
# Kullanım:  ./publish.sh ["commit mesajı"]
#            ./publish.sh --no-push ["commit mesajı"]   # sadece commit, push yok
#
# Gerekli: rustup target add wasm32-unknown-unknown
#          cargo install wasm-bindgen-cli --version 0.2.130
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$REPO_DIR"

PUSH=1
MSG=""
for arg in "$@"; do
  case "$arg" in
    -n|--no-push) PUSH=0 ;;
    *) MSG="$arg" ;;
  esac
done

git pull --ff-only -q || { echo "HATA: git pull basarisiz — once repodaki durumu duzeltin."; exit 1; }

echo "== Rust testleri"
cargo test -q --manifest-path rust/Cargo.toml --workspace
echo "== WebAssembly derleme"
rust/build-web.sh
echo "== PWA denetimi"
node tools/check-pwa.mjs >/dev/null && echo "OK: PWA"

VERSION="$(grep -o '<title>[^<]*</title>' index.html | sed -E 's/.*(v[0-9]+(\.[0-9]+)*).*/\1/')"
[ -n "$MSG" ] || MSG="${VERSION}: commission calculator update"

git add -A
if git diff --cached --quiet; then
  echo "Değişiklik yok (${VERSION:-?}) - push gerekmiyor."
  exit 0
fi
git commit -q -m "$MSG"
if [ "$PUSH" -eq 1 ]; then
  git push
  echo "TAMAM: $MSG"
  echo "Canli: https://sbakbulut.github.io/CommissionApp/"
else
  echo "BILGI: --no-push verildi, commit yerelde kaldi ($MSG)."
fi
