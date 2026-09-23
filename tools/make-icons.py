#!/usr/bin/env python3
"""CommissionApp — PWA ikonlarini ureten arac (saf Python, bagimlilik yok).

Neden var: Chrome'un "Uygulamayi yukle" (install) menusu icin web app manifest'te
192x192 ve 512x512 olmak uzere en az birer ikon ZORUNLU. Bu depoda harici araclar
(ImageMagick/Pillow) olmadan ikon uretilebilsin diye PNG'ler zlib + struct ile
dogrudan uretilir (RGBA, 8-bit, filtre 0).

Uretilen dosyalar (varsayilan hedef: ./icons):
  icon-192.png            : 192x192, purpose="any"      (yuvarlak koseli, seffaf zemin)
  icon-512.png            : 512x512, purpose="any"
  icon-maskable-192.png   : 192x192, purpose="maskable" (tam kaplama + guvenli alan)
  icon-maskable-512.png   : 512x512, purpose="maskable"
  apple-touch-icon.png    : 180x180, iOS "Ana Ekrana Ekle" (tam kaplama)

Kullanim:
  python3 tools/make-icons.py             # ./icons icine yazar ve dogrular
  python3 tools/make-icons.py --out DIR   # baska klasore yazar
  python3 tools/make-icons.py --verify    # uretmez, mevcut PNG'leri cozumleyip denetler

Tasarim: Tiffany mavisi (#0ABAB5) zemin + beyaz yukselen sutunlar (satis/komisyon)
ve marka cizgisi (index.html header .brand-divider dili).
Guvenli alan: maskable ikonlarda icerik merkezde %80 cap'li daire icinde kalir.
"""

from __future__ import annotations

import argparse
import struct
import sys
import zlib
from pathlib import Path

# ── Marka renkleri (index.html :root degiskenleri) ────────────────────────────
BG = (0x0A, 0xBA, 0xB5)  # --tiffany-blue
FG = (0xFF, 0xFF, 0xFF)  # --tiffany-white

SS = 4  # kenar yumusatma icin alt ornekleme (4x4 = 16 ornek/piksel)

# ── Geometri (0..1 normalize koordinat) ───────────────────────────────────────
CORNER_RADIUS = 0.22                      # "any" ikonlarinda yuvarlak kose
BAR_BOTTOM = 0.72                         # sutunlarin taban cizgisi
BARS = ((0.26, 0.10, 0.52), (0.45, 0.10, 0.42), (0.64, 0.10, 0.30))  # x, genislik, tepe
DIVIDER = (0.26, 0.74, 0.775, 0.795)      # x0, x1, y0, y1 — marka cizgisi


def inside_rounded_rect(x: float, y: float, radius: float) -> bool:
    """(x, y) noktasi [0,1]x[0,1] karesinin yuvarlak-koseli hali icinde mi?"""
    if radius <= 0:
        return 0.0 <= x <= 1.0 and 0.0 <= y <= 1.0
    dx = max(radius - x, 0.0, x - (1.0 - radius))
    dy = max(radius - y, 0.0, y - (1.0 - radius))
    return dx * dx + dy * dy <= radius * radius


def in_foreground(x: float, y: float) -> bool:
    """Marka isareti (yukselen sutunlar + cizgi) mi?"""
    if y <= BAR_BOTTOM:
        for bx, bw, btop in BARS:
            if bx <= x <= bx + bw and btop <= y <= BAR_BOTTOM:
                return True
    x0, x1, y0, y1 = DIVIDER
    return x0 <= x <= x1 and y0 <= y <= y1


def render(size: int, rounded: bool, opaque: bool) -> list[bytearray]:
    """RGBA satirlarini uretir (her satir PNG filtre bayti 0 ile yazilir)."""
    radius = CORNER_RADIUS if rounded else 0.0
    rows: list[bytearray] = []
    for py in range(size):
        row = bytearray()
        for px in range(size):
            hits = 0
            r_sum = g_sum = b_sum = 0.0
            for sy in range(SS):
                for sx in range(SS):
                    x = (px + (sx + 0.5) / SS) / size
                    y = (py + (sy + 0.5) / SS) / size
                    if not opaque and not inside_rounded_rect(x, y, radius):
                        continue
                    r, g, b = FG if in_foreground(x, y) else BG
                    r_sum += r
                    g_sum += g
                    b_sum += b
                    hits += 1
            if hits == 0:
                row += bytes((0, 0, 0, 0))  # tamamen seffaf (yuvarlak kose disi)
            else:
                alpha = 255 if opaque else round(255 * hits / samples)
                row += bytes((round(r_sum / hits), round(g_sum / hits), round(b_sum / hits), alpha))
        rows.append(row)
    return rows


def png_bytes(width: int, height: int, rows: list[bytearray]) -> bytes:
    """RGBA satirlarini gecerli bir PNG dosyasina cevirir (CRC'ler dahil)."""
    raw = b"".join(b"\x00" + bytes(row) for row in rows)

    def chunk(tag: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + tag
            + payload
            + struct.pack(">I", zlib.crc32(tag + payload) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)  # 8-bit RGBA
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


TARGETS = (
    # dosya adi, boyut, yuvarlak kose, opak zemin
    ("icon-192.png", 192, True, False),
    ("icon-512.png", 512, True, False),
    ("icon-maskable-192.png", 192, False, True),
    ("icon-maskable-512.png", 512, False, True),
    ("apple-touch-icon.png", 180, False, True),
)


def write_icons(out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    for name, size, rounded, opaque in TARGETS:
        data = png_bytes(size, size, render(size, rounded, opaque))
        (out_dir / name).write_bytes(data)
        print(f"  yazildi: {out_dir / name} ({size}x{size}, {len(data)} bayt)")


def decode_png(path: Path) -> tuple[int, int, list[bytes]]:
    """Kendi urettigimiz PNG'leri (filtre 0) basitce cozer — denetim icin."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path}: PNG imzasi gecersiz")
    pos = 8
    width = height = 0
    idat = b""
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos : pos + 4])
        tag = data[pos + 4 : pos + 8]
        payload = data[pos + 8 : pos + 8 + length]
        (crc,) = struct.unpack(">I", data[pos + 8 + length : pos + 12 + length])
        if crc != (zlib.crc32(tag + payload) & 0xFFFFFFFF):
            raise ValueError(f"{path}: {tag.decode()} CRC gecersiz")
        if tag == b"IHDR":
            width, height, depth, color, _, _, _ = struct.unpack(">IIBBBBB", payload)
            if (depth, color) != (8, 6):
                raise ValueError(f"{path}: 8-bit RGBA bekleniyordu")
        elif tag == b"IDAT":
            idat += payload
        pos += 12 + length
    raw = zlib.decompress(idat)
    stride = width * 4
    if len(raw) != (stride + 1) * height:
        raise ValueError(f"{path}: beklenmeyen veri uzunlugu")
    rows = []
    for y in range(height):
        start = y * (stride + 1)
        if raw[start] != 0:
            raise ValueError(f"{path}: filtre 0 bekleniyordu (satir {y})")
        rows.append(raw[start + 1 : start + 1 + stride])
    return width, height, rows


def pixel(rows: list[bytes], size: int, fx: float, fy: float) -> tuple[int, int, int, int]:
    x = min(size - 1, int(fx * size))
    y = min(size - 1, int(fy * size))
    px = rows[y][x * 4 : x * 4 + 4]
    return px[0], px[1], px[2], px[3]


def verify(out_dir: Path) -> int:
    """Manifest'in istedigi ikonlarin dogru olculerde ve dogru renklerde oldugunu denetler."""
    problems = 0
    for name, size, rounded, opaque in TARGETS:
        path = out_dir / name
        if not path.exists():
            print(f"  HATA  {name}: dosya yok")
            problems += 1
            continue
        width, height, rows = decode_png(path)
        corner = pixel(rows, size, 0.01, 0.01)[3]
        checks = [
            (f"olcu {size}x{size}", (width, height) == (size, size)),
            ("zemin Tiffany mavisi", pixel(rows, size, 0.5, 0.05)[:3] == BG),
            ("sutun beyaz", pixel(rows, size, 0.68, 0.68)[:3] == FG),
            ("kose opak" if opaque else "kose seffaf", corner == 255 if opaque else corner == 0),
        ]
        bad = [label for label, ok in checks if not ok]
        if bad:
            print(f"  HATA  {name}: {'; '.join(bad)}")
            problems += 1
        else:
            print(f"  OK    {name} — {size}x{size}, {len(checks)} kontrol gecti")
    print(f"\n{'TAMAM' if problems == 0 else 'BASARISIZ'}: "
          f"{len(TARGETS) - problems}/{len(TARGETS)} ikon gecerli")
    return 1 if problems else 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="CommissionApp PWA ikon ureteci/denetleyicisi")
    parser.add_argument("--out", default=str(Path(__file__).resolve().parent.parent / "icons"),
                        help="cikti klasoru (varsayilan: <repo>/icons)")
    parser.add_argument("--verify", action="store_true", help="uretmeden mevcut ikonlari denetle")
    args = parser.parse_args(argv)

    out_dir = Path(args.out).resolve()
    if args.verify:
        print(f"Ikon denetimi: {out_dir}")
        return verify(out_dir)
    print(f"Ikon uretiliyor: {out_dir}")
    write_icons(out_dir)
    return verify(out_dir)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
