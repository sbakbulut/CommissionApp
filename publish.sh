#!/usr/bin/env bash
# publish.sh — /sdcard'daki güncel "commission calculator.html" dosyasını repodaki
# index.html'e kopyalar, sürümü <title>'dan okur, commit'ler ve GitHub'a push eder.
#
# Kullanım:  ./publish.sh ["commit mesajı"]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="/sdcard/Termux Deneme/commission calculator.html"
DST="$REPO_DIR/index.html"

# gh CLI PATH'te değilse ~/.local/bin'e bak
command -v gh >/dev/null 2>&1 || export PATH="$HOME/.local/bin:$PATH"
command -v gh >/dev/null 2>&1 || { echo "HATA: gh CLI bulunamadi (https://cli.github.com)"; exit 1; }

[ -f "$SRC" ] || { echo "HATA: kaynak dosya yok: $SRC"; exit 1; }

VERSION="$(grep -o '<title>[^<]*</title>' "$SRC" | sed -E 's/.*(v[0-9]+(\.[0-9]+)*).*/\1/')"
MSG="${1:-$VERSION: commission calculator update}"

cp -f "$SRC" "$DST"
cd "$REPO_DIR"
git add -A
if git diff --cached --quiet; then
  echo "Degisiklik yok ($VERSION) - push gerekmiyor."
  exit 0
fi
git commit -q -m "$MSG"
git push
echo "TAMAM: $MSG"
echo "Canli: https://sbakbulut.github.io/CommissionApp/"
