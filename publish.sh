#!/usr/bin/env bash
# publish.sh — /sdcard'daki güncel "commission calculator.html" dosyasını repodaki
# index.html'e kopyalar, PWA bloklarını yeniden enjekte eder, sürümü <title>'dan okur,
# JS sözdizimini denetler, commit'ler ve GitHub'a push eder.
#
# Kullanım:  ./publish.sh [\"commit mesajı\"]
#            ./publish.sh --no-push [\"commit mesajı\"]   # sadece commit, push yok
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="/sdcard/Termux Deneme/commission calculator.html"
DST="$REPO_DIR/index.html"

PUSH=1
MSG=""
for arg in "$@"; do
  case "$arg" in
    -n|--no-push) PUSH=0 ;;
    *) MSG="$arg" ;;
  esac
done

# gh CLI PATH'te değilse ~/.local/bin'e bak
command -v gh >/dev/null 2>&1 || export PATH="$HOME/.local/bin:$PATH"
command -v gh >/dev/null 2>&1 || { echo "HATA: gh CLI bulunamadi (https://cli.github.com)"; exit 1; }

[ -f "$SRC" ] || { echo "HATA: kaynak dosya yok: $SRC"; exit 1; }

VERSION="$(grep -o '<title>[^<]*</title>' "$SRC" | sed -E 's/.*(v[0-9]+(\.[0-9]+)*).*/\1/')"
[ -n "$MSG" ] || MSG="${VERSION}: commission calculator update"

cp -f "$SRC" "$DST"

# ── PWA bloklarını enjekte et ─────────────────────────────────────────────────
# index.html tek dosyalık uygulamadır; manifest/ikon/service-worker etiketleri ile
# kurulum düğmesi ayrı dosyalarda tutulur (pwa/*.html) ve burada idempotent şekilde
# index.html'e gömülür. Böylece /sdcard'dan gelen dosya üzerine yazsa bile
# Chrome'un "Uygulamayı yükle" ölçütleri korunur.
PWA_HEAD="$REPO_DIR/pwa/head.html"
PWA_BODY="$REPO_DIR/pwa/body-end.html"

inject_block() {   # $1 = blok dosyası, $2 = hedef etiket (regex), $3 = idempotans işareti
  local block="$1" anchor="$2" marker="$3"
  [ -f "$block" ] || { echo "UYARI: PWA blok dosyasi yok: $block"; return 0; }
  if grep -qF "$marker" "$DST"; then
    echo "OK: $marker zaten var (atlandi)"
    return 0
  fi
  local tmp
  tmp="$(mktemp "$DST.XXXXXX")"
  awk -v bf="$block" -v anchor="$anchor" '
    { if (!done && $0 ~ anchor) { while ((getline line < bf) > 0) print line; close(bf); done = 1 } print }
  ' "$DST" > "$tmp" && mv -f "$tmp" "$DST"
  if grep -qF "$marker" "$DST"; then
    echo "OK: PWA blogu eklendi ($marker)"
  else
    echo "UYARI: $marker eklenemedi — '$anchor' satiri bulunamadi."
  fi
}

inject_block "$PWA_HEAD" '</head>' 'PWA:HEAD:START'
inject_block "$PWA_BODY" '</body>' 'PWA:BODY:START'

# ── JS sözdizimi denetimi (node varsa) ────────────────────────────────────────
if command -v node >/dev/null 2>&1; then
  TMP_JS="$(mktemp /tmp/commission-XXXXXX.js)"
  awk '/<script>/{f=1;next} /<\/script>/{f=0} f' "$DST" > "$TMP_JS"
  if ! node --check "$TMP_JS" >/dev/null 2>&1; then
    echo "HATA: inline JS sozdizimi bozuk — commit edilmedi."
    node --check "$TMP_JS" || true
    rm -f "$TMP_JS"
    exit 1
  fi
  rm -f "$TMP_JS"
  echo "OK: JS sozdizimi gecerli (${VERSION:-surum okunamadi})"
else
  echo "UYARI: node bulunamadi, sozdizimi kontrolu atlandi."
fi

cd "$REPO_DIR"
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
  echo "PWA kurulum: Chrome -> menu (:) -> \"Uygulamayi yukle\" ya da uygulama icindeki \"Install app\" dugmesi."
else
  echo "BILGI: --no-push verildi, commit yerelde kaldi ($MSG)."
fi
