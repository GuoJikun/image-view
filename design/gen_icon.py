"""image-view 应用图标生成脚本

产出：
  design/icon-master.svg   矢量母版（1024 画布）
  design/icon-1024.png 等  各尺寸位图
  design/preview.png       设计稿总览（深浅底 + Dock 场景 + 尺寸梯度 + 色板）

依赖：resvg-py（pip install resvg-py）
"""

from __future__ import annotations

import base64
import math
import os

import resvg_py

HERE = os.path.dirname(os.path.abspath(__file__))

# ---------------------------------------------------------------- 几何工具

CANVAS = 1024
BODY = 824.0          # macOS Big Sur 之后的标准图标主体边长
HALF = BODY / 2.0
CENTER = CANVAS / 2.0
N = 5.0               # 超椭圆指数（squircle）


def squircle_points(cx: float, cy: float, a: float, n: float = N, steps: int = 64):
    pts = []
    for i in range(steps):
        t = 2.0 * math.pi * i / steps
        ct, st = math.cos(t), math.sin(t)
        x = cx + a * math.copysign(abs(ct) ** (2.0 / n), ct)
        y = cy + a * math.copysign(abs(st) ** (2.0 / n), st)
        pts.append((x, y))
    return pts


def smooth_closed_path(pts) -> str:
    """闭合 Catmull-Rom -> 三次贝塞尔，得到平滑的超椭圆。"""
    n = len(pts)
    d = [f"M {pts[0][0]:.2f} {pts[0][1]:.2f}"]
    for i in range(n):
        p0, p1 = pts[(i - 1) % n], pts[i]
        p2, p3 = pts[(i + 1) % n], pts[(i + 2) % n]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6.0, p1[1] + (p2[1] - p0[1]) / 6.0)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6.0, p2[1] - (p3[1] - p1[1]) / 6.0)
        d.append(
            f"C {c1[0]:.2f} {c1[1]:.2f} {c2[0]:.2f} {c2[1]:.2f} {p2[0]:.2f} {p2[1]:.2f}"
        )
    return " ".join(d) + " Z"


SQUIRCLE_D = smooth_closed_path(squircle_points(CENTER, CENTER, HALF))

# ---------------------------------------------------------------- 图标

# 相片卡片几何
CARD = dict(x=262.0, y=337.0, w=500.0, h=350.0, r=48.0)

ICON_DEFS = """
  <defs>
    <linearGradient id="bg" x1="0.12" y1="0" x2="0.86" y2="1">
      <stop offset="0" stop-color="#75C4FF"/>
      <stop offset="0.52" stop-color="#4C86FB"/>
      <stop offset="1" stop-color="#3D62E4"/>
    </linearGradient>
    <radialGradient id="gloss" cx="0.32" cy="0.08" r="0.86">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.20"/>
      <stop offset="0.55" stop-color="#FFFFFF" stop-opacity="0.03"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </radialGradient>
    <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#EAF4FF"/>
      <stop offset="0.72" stop-color="#FBFDFF"/>
      <stop offset="1" stop-color="#FFFFFF"/>
    </linearGradient>
    <linearGradient id="near" x1="0.1" y1="0" x2="0.9" y2="1">
      <stop offset="0" stop-color="#86B7FF"/>
      <stop offset="1" stop-color="#5C85EB"/>
    </linearGradient>
    <radialGradient id="sunglow" cx="0.5" cy="0.5" r="0.5">
      <stop offset="0" stop-color="#FFCB72" stop-opacity="0.30"/>
      <stop offset="0.5" stop-color="#FFCB72" stop-opacity="0.06"/>
      <stop offset="1" stop-color="#FFCB72" stop-opacity="0"/>
    </radialGradient>
    <clipPath id="cardClip">
      <rect x="{cx}" y="{cy}" width="{cw}" height="{ch}" rx="{cr}"/>
    </clipPath>
  </defs>
""".format(cx=CARD["x"], cy=CARD["y"], cw=CARD["w"], ch=CARD["h"], cr=CARD["r"])

FRONT_CARD = f"""
  <g>
    <rect x="{CARD['x']}" y="{CARD['y']}" width="{CARD['w']}" height="{CARD['h']}"
          rx="{CARD['r']}" fill="#FFFFFF"/>
  </g>
  <g clip-path="url(#cardClip)">
    <rect x="{CARD['x']}" y="{CARD['y']}" width="{CARD['w']}" height="{CARD['h']}"
          fill="url(#sky)"/>
    <circle cx="356" cy="424" r="60" fill="url(#sunglow)"/>
    <circle cx="356" cy="422" r="32" fill="#FFC260"/>
    <path d="M 262 687
             L 476 472
             L 580 596
             L 660 528
             L 762 687 Z" fill="url(#near)"/>
  </g>
  <rect x="{CARD['x']}" y="{CARD['y']}" width="{CARD['w']}" height="{CARD['h']}"
        rx="{CARD['r']}" fill="none" stroke="#0B2A5B" stroke-opacity="0.09" stroke-width="2"/>
"""


def icon_svg() -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}"
     viewBox="0 0 {CANVAS} {CANVAS}" role="img" aria-label="image-view app icon">
{ICON_DEFS}
  <!-- 图标主体（squircle） -->
  <path d="{SQUIRCLE_D}" fill="url(#bg)"/>
  <path d="{SQUIRCLE_D}" fill="url(#gloss)"/>

  <!-- 相片卡（前层） -->
{FRONT_CARD}

  <!-- 玻璃边缘高光 -->
  <path d="{SQUIRCLE_D}" fill="none" stroke="#FFFFFF" stroke-opacity="0.18" stroke-width="2.5"/>
</svg>
"""


# ---------------------------------------------------------------- 小图标（Dock 占位）

def mini_icon(x: float, y: float, size: float, c1: str, c2: str, glyph: str, key: str) -> str:
    a = size / 2.0
    d = smooth_closed_path(squircle_points(x + a, y + a, a * 0.858, steps=24))
    return f"""
  <g>
    <path d="{d}" fill="url(#g_{key})"/>
    <path d="{d}" fill="none" stroke="#FFFFFF" stroke-opacity="0.22" stroke-width="{size*0.012:.1f}"/>
    {glyph}
  </g>
  <defs>
    <linearGradient id="g_{key}" x1="0.2" y1="0" x2="0.85" y2="1">
      <stop offset="0" stop-color="{c1}"/>
      <stop offset="1" stop-color="{c2}"/>
    </linearGradient>
  </defs>"""


def glyph(kind: str, x: float, y: float, s: float) -> str:
    cx, cy = x + s / 2, y + s / 2
    if kind == "circle":
        r = s * 0.20
        return f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="#FFFFFF" fill-opacity="0.88"/>'
    if kind == "square":
        r = s * 0.34
        return (f'<rect x="{cx - r/2:.1f}" y="{cy - r/2:.1f}" width="{r:.1f}" height="{r:.1f}" '
                f'rx="{r*0.26:.1f}" fill="#FFFFFF" fill-opacity="0.88"/>')
    if kind == "bars":
        w = s * 0.09
        out = []
        for i, hh in enumerate((0.30, 0.46, 0.36)):
            bx = cx - s * 0.22 + i * s * 0.19
            out.append(f'<rect x="{bx:.1f}" y="{cy - s*hh/2:.1f}" width="{w:.1f}" '
                       f'height="{s*hh:.1f}" rx="{w/2:.1f}" fill="#FFFFFF" fill-opacity="0.88"/>')
        return "".join(out)
    if kind == "play":
        r = s * 0.26
        p = f"{cx - r*0.55:.1f},{cy - r:.1f} {cx + r*0.85:.1f},{cy:.1f} {cx - r*0.55:.1f},{cy + r:.1f}"
        return f'<polygon points="{p}" fill="#FFFFFF" fill-opacity="0.88"/>'
    r = s * 0.34
    p = (f"{cx:.1f},{cy - r:.1f} {cx + r*0.95:.1f},{cy + r*0.7:.1f} "
         f"{cx - r*0.95:.1f},{cy + r*0.7:.1f}")
    return f'<polygon points="{p}" fill="#FFFFFF" fill-opacity="0.88"/>'


# ---------------------------------------------------------------- 预览稿

def b64_png(path: str) -> str:
    with open(path, "rb") as f:
        return base64.b64encode(f.read()).decode("ascii")


def preview_svg(icon_png_uri: str) -> str:
    W, H = 1600, 1050
    parts: list[str] = []

    # 背景
    parts.append(f'<rect width="{W}" height="{H}" fill="#F1F3F7"/>')
    parts.append("""
  <defs>
    <filter id="soft" x="-30%" y="-30%" width="160%" height="170%">
      <feDropShadow dx="0" dy="10" stdDeviation="18" flood-color="#14213D" flood-opacity="0.10"/>
    </filter>
    <filter id="dockShadow" x="-20%" y="-40%" width="140%" height="200%">
      <feDropShadow dx="0" dy="12" stdDeviation="20" flood-color="#050B18" flood-opacity="0.45"/>
    </filter>
    <linearGradient id="wall" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#2B3A67"/>
      <stop offset="0.5" stop-color="#3C2E63"/>
      <stop offset="1" stop-color="#1D2A4A"/>
    </linearGradient>
  </defs>""")

    # 标题
    parts.append(
        '<text x="100" y="112" font-family="Microsoft YaHei, Segoe UI, Helvetica, Arial, sans-serif" '
        'font-size="52" font-weight="700" fill="#0F172A" letter-spacing="-1">image-view</text>'
    )
    parts.append(
        '<text x="100" y="160" font-family="Microsoft YaHei, Segoe UI, Helvetica, Arial, sans-serif" '
        'font-size="21" fill="#6B7A90">应用图标设计 · macOS 简洁风格 · squircle 1024 × 1024</text>'
    )
    parts.append(
        '<text x="1500" y="112" text-anchor="end" font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" '
        'font-size="18" fill="#94A3B8">v1.0</text>'
    )

    # 左右两张大卡：浅底 / 深底
    for bx, fill, stroke, label in (
        (100, "#FFFFFF", "#E3E8EF", "浅色背景"),
        (570, "#0B1220", "#1B2536", "深色背景"),
    ):
        parts.append(
            f'<rect x="{bx}" y="212" width="430" height="424" rx="30" fill="{fill}" '
            f'stroke="{stroke}" stroke-width="1.5" filter="url(#soft)"/>'
        )
        parts.append(f'<image x="{bx + 65}" y="256" width="300" height="300" href="{icon_png_uri}"/>')
        parts.append(
            f'<text x="{bx + 215}" y="606" text-anchor="middle" '
            'font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" font-size="17" '
            f'fill="{"#94A3B8" if fill != "#FFFFFF" else "#7A8899"}">{label}</text>'
        )

    # 右列：尺寸梯度
    parts.append(
        '<text x="1050" y="248" font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" '
        'font-size="19" font-weight="700" fill="#334155">尺寸适配</text>'
    )
    ladder = [(128, 276), (64, 424), (32, 508), (16, 560)]
    for size, y in ladder:
        parts.append(f'<image x="1050" y="{y}" width="{size}" height="{size}" href="{icon_png_uri}"/>')
        parts.append(
            f'<text x="{1050 + size + 22}" y="{y + size / 2 + 6}" '
            'font-family="Consolas, monospace" font-size="15" fill="#8B98A9">'
            f'{size} px</text>'
        )


    # 色板
    parts.append(
        '<text x="1330" y="248" font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" '
        'font-size="19" font-weight="700" fill="#334155">主色板</text>'
    )
    swatches = [("#75C4FF", "高光蓝"), ("#4C86FB", "主色蓝"), ("#3D62E4", "深影蓝"),
                ("#FFC260", "日光橙"), ("#5C85EB", "山峦蓝"), ("#FFFFFF", "卡片白")]
    for i, (hexv, name) in enumerate(swatches):
        col, row = i % 2, i // 2
        sx, sy = 1330 + col * 90, 276 + row * 112
        parts.append(
            f'<rect x="{sx}" y="{sy}" width="60" height="60" rx="16" fill="{hexv}" '
            f'stroke="#DDE3EA" stroke-width="1"/>'
        )
        parts.append(
            f'<text x="{sx + 30}" y="{sy + 78}" text-anchor="middle" '
            'font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" font-size="12.5" '
            'fill="#8E9BAB">'
            f'{name}</text>'
        )

    # Dock 场景
    parts.append(
        '<rect x="100" y="686" width="1400" height="212" rx="28" fill="url(#wall)"/>'
    )
    parts.append(
        '<text x="132" y="730" font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" '
        'font-size="15" fill="#FFFFFF" fill-opacity="0.55" letter-spacing="2">DOCK 场景</text>'
    )
    dock_x, dock_y, dock_w, dock_h = 300, 748, 1000, 104
    parts.append(
        f'<rect x="{dock_x}" y="{dock_y}" width="{dock_w}" height="{dock_h}" rx="28" '
        'fill="#FFFFFF" fill-opacity="0.14" stroke="#FFFFFF" stroke-opacity="0.26" '
        'stroke-width="1.5" filter="url(#dockShadow)"/>'
    )
    others = [
        ("#FF9E7E", "#F2645A", "circle"),
        ("#FFD479", "#F4A020", "square"),
        ("#9CE7B0", "#3FC982", "bars"),
        ("#B5C9FF", "#6478F0", "play"),
        ("#FFB6D5", "#F065A3", "triangle"),
        ("#9FE6E6", "#35BFBF", "circle"),
        ("#C6B6FF", "#8B6BF0", "square"),
        ("#FFC9A0", "#F09050", "bars"),
    ]
    isz, gap = 74, 14
    total = len(others) * isz + (len(others) - 1) * gap + isz + gap
    start = dock_x + (dock_w - total) / 2
    idx_self = 4
    x = start
    oi = 0
    for i in range(len(others) + 1):
        if i == idx_self:
            parts.append(f'<image x="{x:.1f}" y="{dock_y + (dock_h - isz) / 2}" '
                         f'width="{isz}" height="{isz}" href="{icon_png_uri}"/>')
            parts.append(f'<circle cx="{x + isz / 2:.1f}" cy="{dock_y + dock_h - 9}" r="3.5" '
                         'fill="#FFFFFF" fill-opacity="0.7"/>')
        else:
            c1, c2, gk = others[oi]
            oi += 1
            parts.append(mini_icon(x, dock_y + (dock_h - isz) / 2, isz, c1, c2,
                                   glyph(gk, x, dock_y + (dock_h - isz) / 2, isz), f"d{i}"))
        x += isz + gap

    # 页脚
    parts.append(
        '<text x="100" y="980" font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" '
        'font-size="15" fill="#A6B1BF">母版 design/icon-master.svg · '
        '位图 design/icon-128…1024.png</text>'
    )
    parts.append(
        '<text x="1500" y="980" text-anchor="end" '
        'font-family="Microsoft YaHei, Segoe UI, Arial, sans-serif" font-size="15" '
        'fill="#A6B1BF">image-view · design</text>'
    )

    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" '
            f'viewBox="0 0 {W} {H}">' + "".join(parts) + "</svg>")


# ---------------------------------------------------------------- 输出

def write_svg(path: str, svg: str) -> None:
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(svg)


def render(svg: str, out: str, width: int | None = None) -> None:
    data = resvg_py.svg_to_bytes(svg_string=svg, width=width)
    with open(out, "wb") as f:
        f.write(bytes(data))
    print(f"  -> {os.path.relpath(out, HERE)}")


def main() -> None:
    os.makedirs(HERE, exist_ok=True)
    icon = icon_svg()
    write_svg(os.path.join(HERE, "icon-master.svg"), icon)

    master = os.path.join(HERE, "icon-1024.png")
    print("渲染图标：")
    render(icon, master, 1024)
    for s in (512, 256, 128, 64, 32, 16):
        render(icon, os.path.join(HERE, f"icon-{s}.png"), s)

    print("渲染预览：")
    uri = "data:image/png;base64," + b64_png(master)
    prev = preview_svg(uri)
    write_svg(os.path.join(HERE, "preview.svg"), prev)
    render(prev, os.path.join(HERE, "preview.png"), 1600)


if __name__ == "__main__":
    main()
