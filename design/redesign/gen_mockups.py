"""image-view 界面改版设计稿生成脚本（仅设计，不涉及应用代码）。

产出（design/redesign/*.png）：
  01-toolbar-light.png     版式 A：工具栏模式（无菜单栏）· 浅色
  02-toolbar-dark.png      版式 A：工具栏模式（无菜单栏）· 深色
  03-menu-windows.png      版式 B：Windows 原生菜单栏
  04-menu-macos.png        版式 B：macOS 系统菜单栏
  05-spec.png              规格页：色板、尺寸、图标、状态、三端差异、空状态

依赖：resvg-py
"""

from __future__ import annotations

import base64
import os

import resvg_py

HERE = os.path.dirname(os.path.abspath(__file__))
DESIGN = os.path.dirname(HERE)
ROOT = os.path.dirname(DESIGN)

FONT = "Segoe UI, Microsoft YaHei, PingFang SC, Helvetica Neue, Arial, sans-serif"

PAGE_W = 1600
PAGE_H = 1080

# 窗口几何：标题栏 38 / 工具栏 52 / 缩略图条 96 / 状态栏 26
WIN_X = 80
WIN_Y = 60
WIN_W = 1440
WIN_H = 880
TITLEBAR_H = 38
TOOLBAR_H = 52
STRIP_H = 88
STATUS_H = 26
MENUBAR_H = 22

CANVAS_Y = WIN_Y + TITLEBAR_H + TOOLBAR_H
CANVAS_BOTTOM = WIN_Y + WIN_H - STRIP_H - STATUS_H

LIGHT = {
    "page": "#E3E3E7",
    "chrome": "#F6F6F7",
    "border": "#D9D9DC",
    "hairline": "#E4E4E7",
    "text": "#1D1D1F",
    "text2": "#6E6E73",
    "text3": "#9A9AA0",
    "icon": "#48484C",
    "hover": "rgba(0,0,0,0.055)",
    "press": "rgba(0,0,0,0.10)",
    "control": "#FFFFFF",
    "control_border": "rgba(0,0,0,0.10)",
    "track": "#E9E9EB",
    "accent": "#0A7CFF",
    "accent_soft": "rgba(10,124,255,0.13)",
    "canvas": "#1C1C1E",
    "strip": "#EFEFF1",
    "status": "#F6F6F7",
    "danger": "#E5484D",
    "shadow": "rgba(0,0,0,0.22)",
    "tile": "#FFFFFF",
    "tile_border": "rgba(0,0,0,0.07)",
}

DARK = {
    "page": "#151517",
    "chrome": "#2A2A2D",
    "border": "#3A3A3D",
    "hairline": "#3A3A3D",
    "text": "#F5F5F7",
    "text2": "#A1A1A6",
    "text3": "#7C7C82",
    "icon": "#D3D3D9",
    "hover": "rgba(255,255,255,0.07)",
    "press": "rgba(255,255,255,0.13)",
    "control": "#3A3A3D",
    "control_border": "rgba(255,255,255,0.10)",
    "track": "#3A3A3D",
    "accent": "#0A84FF",
    "accent_soft": "rgba(10,132,255,0.20)",
    "canvas": "#101012",
    "strip": "#232326",
    "status": "#2A2A2D",
    "danger": "#FF6961",
    "shadow": "rgba(0,0,0,0.55)",
    "tile": "#333338",
    "tile_border": "rgba(255,255,255,0.07)",
}


# ---------------------------------------------------------------- 基础绘图

def rect(x, y, w, h, rad=0.0, fill="none", stroke=None, sw=1.0, opacity=None, extra=""):
    out = f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}"'
    if rad:
        out += f' rx="{rad:.1f}"'
    out += f' fill="{fill}"'
    if stroke:
        out += f' stroke="{stroke}" stroke-width="{sw:.1f}"'
    if opacity is not None:
        out += f' opacity="{opacity:.2f}"'
    return out + f" {extra}/>"


def path(d, stroke="none", sw=1.5, fill="none", cap="round", join="round", opacity=None):
    out = f'<path d="{d}" fill="{fill}" stroke="{stroke}" stroke-width="{sw:.2f}"'
    out += f' stroke-linecap="{cap}" stroke-linejoin="{join}"'
    if opacity is not None:
        out += f' opacity="{opacity:.2f}"'
    return out + "/>"


def circle(cx, cy, r, fill="none", stroke=None, sw=1.0):
    out = f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{r:.1f}" fill="{fill}"'
    if stroke:
        out += f' stroke="{stroke}" stroke-width="{sw:.1f}"'
    return out + "/>"


def line(x1, y1, x2, y2, stroke, sw=1.0, opacity=None):
    out = f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" stroke="{stroke}" stroke-width="{sw:.1f}"'
    if opacity is not None:
        out += f' opacity="{opacity:.2f}"'
    return out + "/>"


def text(x, y, s, size=13, fill="#000", weight=400, anchor="start", opacity=None, spacing=None):
    s = (
        s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
    )
    out = (
        f'<text x="{x:.1f}" y="{y:.1f}" font-family="{FONT}" font-size="{size}" '
        f'font-weight="{weight}" fill="{fill}" text-anchor="{anchor}"'
    )
    if opacity is not None:
        out += f' opacity="{opacity:.2f}"'
    if spacing:
        out += f' letter-spacing="{spacing}"'
    return out + f">{s}</text>"


def image(x, y, w, h, data, rad=0.0, opacity=None):
    out = (
        f'<image x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" '
        f'xlink:href="data:image/png;base64,{data}" preserveAspectRatio="xMidYMid slice"'
    )
    if rad:
        out += f' clip-path="url(#imgclip{int(rad)})"'
    if opacity is not None:
        out += f' opacity="{opacity:.2f}"'
    return out + "/>"


def read_png(name: str) -> str:
    with open(os.path.join(DESIGN, name), "rb") as handle:
        return base64.b64encode(handle.read()).decode()


# ---------------------------------------------------------------- 图标（16×16）

ICONS = {
    # 打开：文件夹
    "folder": [
        'M2.4 4.8c0-.9.7-1.6 1.6-1.6h2.2l1.4 1.6h4.4c.9 0 1.6.7 1.6 1.6v5.6c0 .9-.7 1.6-1.6 1.6H4c-.9 0-1.6-.7-1.6-1.6z',
    ],
    "chevron-left": ['M9.9 3.6 5.7 8l4.2 4.4'],
    "chevron-right": ['M6.1 3.6 10.3 8l-4.2 4.4'],
    "chevron-down": ['M4.4 6.4 8 9.9l3.6-3.5'],
    "minus": ['M3.8 8h8.4'],
    "plus": ['M3.8 8h8.4', 'M8 3.8v8.4'],
    "play": ['M5.6 3.9 12.1 8 5.6 12.1z'],
    "pause": ['M6.2 4.4v7.2', 'M9.8 4.4v7.2'],
    "stack": [
        'M3 6.6c0-.8.6-1.4 1.4-1.4h4.2c.8 0 1.4.6 1.4 1.4v4.4c0 .8-.6 1.4-1.4 1.4H4.4c-.8 0-1.4-.6-1.4-1.4z',
        'M6.4 3.2h5.2c.8 0 1.4.6 1.4 1.4v5.2',
    ],
    "sort": [
        'M5.1 12.4V4.1',
        'M5.1 4.1 2.7 6.5',
        'M5.1 4.1 7.5 6.5',
        'M10.9 3.6v8.3',
        'M10.9 11.9 8.5 9.5',
        'M10.9 11.9 13.3 9.5',
    ],
    "ellipsis": [],
    "help": ['M6.1 6.1a1.9 1.9 0 1 1 2.55 1.78c-.51.2-.85.63-.85 1.17v.42', 'M8 11.9v.01'],
    "reveal": ['M3.2 4.6h9.6v8.2H3.2z', 'M6.6 9.4 10 6', 'M7.4 6h2.6v2.6'],
    "copy": [
        'M10.3 5.4V3.8c0-.6-.5-1.1-1.1-1.1H4.3c-.6 0-1.1.5-1.1 1.1v5.4c0 .6.5 1.1 1.1 1.1h1.6',
        'M6.1 5.8h5.5c.6 0 1.1.5 1.1 1.1v5.4c0 .6-.5 1.1-1.1 1.1H6.1c-.6 0-1.1-.5-1.1-1.1V6.9c0-.6.5-1.1 1.1-1.1z',
    ],
    "trash": [
        'M3.4 5h9.2',
        'M6.2 5V3.6h3.6V5',
        'M4.7 5l.5 7.2c.05.6.55 1.1 1.15 1.1h3.3c.6 0 1.1-.5 1.15-1.1L11.3 5',
    ],
    "sliders": ['M3.4 6h9.2', 'M3.4 10.4h9.2', 'M6.4 4.4v3.2', 'M10 8.8v3.2'],
    "sidebar": [
        'M4 3.4h8c1 0 1.8.8 1.8 1.8v5.6c0 1-.8 1.8-1.8 1.8H4c-1 0-1.8-.8-1.8-1.8V5.2c0-1 .8-1.8 1.8-1.8z',
        'M7.2 3.5v8.9',
    ],
    "warning": ['M8 3.1 14.3 13H1.7z', 'M8 7.2v2.9', 'M8 11.6v.01'],
}


def icon(name, x, y, color, size=16.0, sw=1.5):
    scale = size / 16.0
    parts = []
    for d in ICONS[name]:
        parts.append(path(d, stroke=color, sw=sw))
    if name == "help":
        parts.insert(0, circle(8.0, 8.0, 6.2, stroke=color, sw=sw))
    if name == "ellipsis":
        for cx in (3.5, 8.0, 12.5):
            parts.append(circle(cx, 8.0, 1.15, fill=color))
    if name == "play":
        parts = [path(ICONS["play"][0], fill=color, stroke=color, sw=0.8)]
    if name == "pause":
        parts = [rect(6.1, 4.4, 1.5, 7.2, 0.75, fill=color), rect(9.5, 4.4, 1.5, 7.2, 0.75, fill=color)]
    return f'<g transform="translate({x:.1f},{y:.1f}) scale({scale:.4f})">{"".join(parts)}</g>'


# ---------------------------------------------------------------- 控件

def icon_button(x, y, name, theme, w=30.0, h=30.0, state="default", tint=None, size=17.0):
    color = tint or theme["icon"]
    fill = "none"
    if state == "hover":
        fill = theme["hover"]
    elif state == "press":
        fill = theme["press"]
    elif state == "on":
        fill = theme["hover"]
        color = tint or theme["text"]
    out = rect(x, y, w, h, 6.0, fill=fill)
    out += icon(name, x + (w - size) / 2, y + (h - size) / 2, color, size)
    return out


def text_button(x, y, label, theme, w, h=28.0, state="default", chevron=True):
    fill = theme["hover"] if state == "hover" else "none"
    out = rect(x, y, w, h, 6.0, fill=fill)
    out += text(x + 9, y + h / 2 + 4.2, label, size=12.0, fill=theme["text"], weight=500)
    if chevron:
        out += icon("chevron-down", x + w - 17, y + (h - 12) / 2, theme["text2"], 12.0, 1.4)
    return out


def segmented_zoom(x, y, value, theme, w=118.0, h=30.0):
    out = rect(x, y, w, h, 6.0, fill=theme["track"], stroke=theme["control_border"], sw=1.0)
    seg = w / 3.0
    out += icon("minus", x + (seg - 16) / 2, y + (h - 16) / 2, theme["icon"], 16)
    out += icon("plus", x + w - seg + (seg - 16) / 2, y + (h - 16) / 2, theme["icon"], 16)
    # 中段：白底“选中”外观
    out += rect(x + seg + 1.5, y + 2, seg - 3, h - 4, 4.5, fill=theme["control"], stroke=theme["control_border"], sw=1.0)
    out += text(x + w / 2, y + h / 2 + 4.0, value, size=12.0, fill=theme["text"], weight=500, anchor="middle")
    return out


def divider(x, y, h=18.0, theme=None):
    return line(x, y, x, y + h, theme["border"])


# ---------------------------------------------------------------- 窗口部件

def titlebar_macos(theme, y, title):
    out = rect(WIN_X, y, WIN_W, TITLEBAR_H, 0, fill=theme["chrome"])
    for i, color in enumerate(("#FF5F57", "#FEBC2E", "#28C840")):
        out += circle(WIN_X + 20 + i * 20, y + TITLEBAR_H / 2, 6.0, fill=color)
    out += text(WIN_X + WIN_W / 2, y + TITLEBAR_H / 2 + 4.5, title, size=12.5, fill=theme["text"], weight=500, anchor="middle")
    return out


def titlebar_windows(theme, y, title):
    out = rect(WIN_X, y, WIN_W, TITLEBAR_H, 0, fill=theme["chrome"])
    out += rect(WIN_X + 16, y + 11, 16, 16, 3.5, fill=theme["accent_soft"])
    out += icon("stack", WIN_X + 17, y + 12, theme["accent"], 14, 1.4)
    out += text(WIN_X + 40, y + TITLEBAR_H / 2 + 4.4, title, size=12.0, fill=theme["text2"], weight=400)
    # 右侧窗口按钮
    bx = WIN_X + WIN_W
    out += line(bx - 124, y + TITLEBAR_H / 2, bx - 112, y + TITLEBAR_H / 2, theme["icon"], 1.1)
    out += rect(bx - 90, y + TITLEBAR_H / 2 - 5, 10, 10, 1.2, stroke=theme["icon"], sw=1.1)
    out += path(f"M{bx - 68} {y + TITLEBAR_H / 2 - 5} l10 10 M{bx - 58} {y + TITLEBAR_H / 2 - 5} l-10 10", stroke=theme["icon"], sw=1.1)
    return out


def toolbar(theme, y, *, has_image=True, zoom="46%", slideshow=False, thumbnails=True, variant="A", center_title=None):
    out = rect(WIN_X, y, WIN_W, TOOLBAR_H, 0, fill=theme["chrome"])
    out += line(WIN_X, y + TOOLBAR_H, WIN_X + WIN_W, y + TOOLBAR_H, theme["border"], 1.0)

    cy = y + (TOOLBAR_H - 30) / 2
    x = WIN_X + 12

    if variant == "A":
        out += icon_button(x, cy, "folder", theme)
        x += 30 + 10
        out += divider(x + 1, y + 17, 18, theme)
        x += 12

    prev_color = theme["icon"] if has_image else theme["text3"]
    out += icon_button(x, cy, "chevron-left", theme, tint=prev_color)
    out += icon_button(x + 30, cy, "chevron-right", theme, tint=prev_color)
    left_end = x + 30 + 30

    # 右侧分组（从右往左排布）
    right = WIN_X + WIN_W - 12
    if variant == "A":
        out += icon_button(right - 30, cy, "ellipsis", theme)
        right -= 30 + 12
    out += segmented_zoom(right - 118, cy, zoom, theme)
    right -= 118 + 12
    out += divider(right + 3, y + 17, 18, theme)
    right -= 9
    out += text_button(right - 62, cy, "名称", theme, 62)
    right -= 62 + 4
    out += icon_button(right - 30, cy, "play" if not slideshow else "pause", theme,
                       state="press" if slideshow else "default",
                       tint=theme["accent"] if slideshow else None)
    right -= 30 + 4
    out += icon_button(right - 30, cy, "sidebar", theme, state="on" if thumbnails else "default",
                       tint=theme["accent"] if thumbnails else None)

    # 中央留白：文件名交给系统标题栏；全屏（无标题栏）时才在中央显示
    if center_title:
        title_left = left_end + 16
        title_right = right - 8
        out += text((title_left + title_right) / 2, y + TOOLBAR_H / 2 + 4.6, center_title,
                    size=13.0, fill=theme["text"], weight=500, anchor="middle")
    return out


def canvas(theme, y0, y1, preview_data):
    cw = WIN_W
    ch = y1 - y0
    out = rect(WIN_X, y0, cw, ch, 0, fill=theme["canvas"])
    # 图片：按比例内缩，带投影
    img_w = ch * 1.524 * 0.90
    img_h = ch * 0.90
    ix = WIN_X + (cw - img_w) / 2
    iy = y0 + (ch - img_h) / 2
    out += rect(ix + 1, iy + 6, img_w, img_h, 3, fill="#000000", opacity=0.35)
    out += rect(ix, iy, img_w, img_h, 2.5, fill="#FFFFFF")
    out += f'<g clip-path="url(#shotclip)">'
    out += image(ix, iy, img_w, img_h, preview_data, 2.5)
    out += "</g>"
    out += rect(ix, iy, img_w, img_h, 2.5, stroke="rgba(255,255,255,0.10)", sw=1.0)
    return out


def strip(theme, y, count=10, current=8):
    shot = read_png("preview.png")
    out = rect(WIN_X, y, WIN_W, STRIP_H, 0, fill=theme["strip"])
    out += line(WIN_X, y, WIN_X + WIN_W, y, theme["border"], 1.0)

    tw, th, gap = 76.0, 56.0, 9.0
    total = count * tw + (count - 1) * gap
    x0 = WIN_X + (WIN_W - total) / 2
    ty = y + 16

    # 圆角裁切：每个卡片单独一个 clipPath
    out += "<defs>"
    for i in range(count):
        cx = x0 + i * (tw + gap)
        out += f'<clipPath id="tileclip{i}"><rect x="{cx:.1f}" y="{ty:.1f}" width="{tw:.1f}" height="{th:.1f}" rx="6"/></clipPath>'
    out += "</defs>"

    for i in range(count):
        x = x0 + i * (tw + gap)
        selected = i == current
        out += rect(x, ty, tw, th, 6.0, fill=theme["tile"])
        out += (
            f'<g clip-path="url(#tileclip{i})">'
            f'<image x="{x:.1f}" y="{ty:.1f}" width="{tw:.1f}" height="{th:.1f}" '
            f'xlink:href="data:image/png;base64,{shot}" preserveAspectRatio="xMidYMid slice"/>'
            "</g>"
        )
        out += rect(x + 0.5, ty + 0.5, tw - 1, th - 1, 6.0, fill="none", stroke=theme["tile_border"], sw=1.0)
        if selected:
            out += rect(x - 3.5, ty - 3.5, tw + 7, th + 7, 9.0, fill="none", stroke=theme["accent"], sw=2.0)
    return out


def status(theme, y):
    out = rect(WIN_X, y, WIN_W, STATUS_H, 0, fill=theme["status"])
    out += line(WIN_X, y, WIN_X + WIN_W, y, theme["border"], 1.0)
    base = y + STATUS_H / 2 + 3.8
    out += text(WIN_X + 14, base, "1600 × 1050", size=11.0, fill=theme["text2"])
    out += text(WIN_X + 82, base, "·", size=11.0, fill=theme["text3"])
    out += text(WIN_X + 92, base, "PNG", size=11.0, fill=theme["text3"])
    out += text(WIN_X + 128, base, "·", size=11.0, fill=theme["text3"])
    out += text(WIN_X + 138, base, "sRGB", size=11.0, fill=theme["text3"])
    out += text(WIN_X + WIN_W - 14, base, "9 / 10", size=11.0, fill=theme["text2"], anchor="end")
    return out


def empty_state(theme, x, y, w, h):
    out = rect(x, y, w, h, 0, fill=theme["canvas"])
    cx = x + w / 2
    cy = y + h / 2
    out += rect(cx - 34, cy - 74, 68, 68, 16, fill="rgba(255,255,255,0.07)", stroke="rgba(255,255,255,0.12)", sw=1)
    out += icon("stack", cx - 16, cy - 56, "rgba(255,255,255,0.55)", 32, 1.4)
    out += text(cx, cy + 16, "拖入图片，或按 Ctrl+O 打开", size=14, fill="rgba(255,255,255,0.72)", anchor="middle")
    bw, bh = 132, 32
    out += rect(cx - bw / 2, cy + 34, bw, bh, 7, fill=theme["accent"])
    out += text(cx, cy + 34 + bh / 2 + 4.5, "打开图片…", size=12.5, fill="#FFFFFF", weight=500, anchor="middle")
    return out


def shadow(theme):
    out = f'<g filter="url(#winShadow)">'
    out += rect(WIN_X, WIN_Y, WIN_W, WIN_H, 11, fill=theme["chrome"])
    out += "</g>"
    return out


def page(theme, body, caption, note, top=""):
    out = f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{PAGE_W}" height="{PAGE_H}" viewBox="0 0 {PAGE_W} {PAGE_H}">'
    out += "<defs>"
    out += (
        '<filter id="winShadow" x="-20%" y="-20%" width="140%" height="150%">'
        '<feDropShadow dx="0" dy="14" stdDeviation="18" flood-color="#000000" flood-opacity="0.22"/>'
        "</filter>"
    )
    out += f'<clipPath id="imgclip2"><rect x="0" y="0" width="{PAGE_W}" height="{PAGE_H}"/></clipPath>'
    out += "</defs>"
    out += rect(0, 0, PAGE_W, PAGE_H, 0, fill=theme["page"])
    out += top
    out += body
    out += text(WIN_X, PAGE_H - 34, caption, size=12.5, fill=theme["text2"])
    out += text(WIN_X + WIN_W, PAGE_H - 34, note, size=12.5, fill=theme["text3"], anchor="end")
    out += "</svg>"
    return out


def window_shell(theme, body_inner):
    clip = (
        f'<clipPath id="winclip"><rect x="{WIN_X}" y="{WIN_Y}" width="{WIN_W}" height="{WIN_H}" rx="11"/></clipPath>'
    )
    return shadow(theme) + clip + f'<g clip-path="url(#winclip)">{body_inner}</g>' + rect(
        WIN_X, WIN_Y, WIN_W, WIN_H, 11, fill="none", stroke=theme["border"], sw=1.0
    )


# ---------------------------------------------------------------- 各版式

def body_toolbar(theme, title, variant="A"):
    preview = read_png("preview.png")
    y = WIN_Y
    body = ""
    body += titlebar_macos(theme, y, title)
    body += toolbar(theme, y + TITLEBAR_H, variant=variant)
    body += canvas(theme, CANVAS_Y, CANVAS_BOTTOM, preview)
    body += strip(theme, CANVAS_BOTTOM)
    body += status(theme, CANVAS_BOTTOM + STRIP_H)
    body += rect(WIN_X, WIN_Y, WIN_W, WIN_H, 11, fill="none", stroke=theme["border"], sw=1.0)
    return window_shell(theme, body)


def mock_toolbar_windows(theme, caption):
    """版式 A 在 Windows 外壳下的样子：无菜单栏，全部入口在工具栏。"""
    preview = read_png("preview.png")
    y = WIN_Y
    body = titlebar_windows(theme, y, "preview.png — 图片查看器")
    body += toolbar(theme, y + TITLEBAR_H, variant="A")
    body += canvas(theme, CANVAS_Y, CANVAS_BOTTOM, preview)
    body += strip(theme, CANVAS_BOTTOM)
    body += status(theme, CANVAS_BOTTOM + STRIP_H)
    return page(theme, window_shell(theme, body), caption, "版式 A · Windows 外壳（无菜单栏，推荐默认）")


def mock_toolbar(theme, title, caption):
    return page(theme, body_toolbar(theme, title), caption, "版式 A · 工具栏模式（无菜单栏）")


def closeup(theme, x, y, w, h, inner, name):
    svg = (
        f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" '
        f'width="{w}" height="{h}" viewBox="0 0 {w} {h}">'
    )
    svg += (
        '<defs><filter id="winShadow" x="-20%" y="-20%" width="140%" height="150%">'
        '<feDropShadow dx="0" dy="14" stdDeviation="18" flood-color="#000000" flood-opacity="0.22"/>'
        "</filter></defs>"
    )
    svg += rect(0, 0, w, h, 0, fill=theme["page"])
    svg += f'<g transform="translate({-x},{-y})">{inner}</g>'
    svg += "</svg>"
    render(svg, name)


def macos_system_bar(theme):
    out = rect(0, 0, PAGE_W, 30, 0, fill="#F2F2F4")
    out += line(0, 30, PAGE_W, 30, "#D8D8DC", 1.0)
    out += image(22, 7, 16, 16, read_png("icon-32.png"))
    out += text(48, 20, "image-view", size=12.5, fill="#1D1D1F", weight=600)
    x = 136
    for label in ("文件", "编辑", "显示", "窗口", "帮助"):
        out += text(x, 20, label, size=12.5, fill="#1D1D1F")
        x += 62
    out += text(PAGE_W - 24, 20, "周三 11:05", size=12, fill="#3C3C43", opacity=0.75, anchor="end")
    return out


def mock_menu_windows(theme, caption):
    preview = read_png("preview.png")
    y = WIN_Y
    menubar_y = y + TITLEBAR_H
    body = titlebar_windows(theme, y, "preview.png — 图片查看器")
    # 原生菜单栏（Win32）
    body += rect(WIN_X, menubar_y, WIN_W, MENUBAR_H, 0, fill=theme["control"])
    body += line(WIN_X, menubar_y + MENUBAR_H, WIN_X + WIN_W, menubar_y + MENUBAR_H, theme["border"], 1.0)
    mx = WIN_X + 12
    for label in ("文件(F)", "编辑(E)", "查看(V)", "帮助(H)"):
        body += text(mx, menubar_y + 15.5, label, size=12.0, fill=theme["text"])
        mx += len(label) * 12.4 + 14
    tool_y = menubar_y + MENUBAR_H
    body += toolbar(theme, tool_y, variant="B")
    body += canvas(theme, tool_y + TOOLBAR_H, CANVAS_BOTTOM, preview)
    body += strip(theme, CANVAS_BOTTOM)
    body += status(theme, CANVAS_BOTTOM + STRIP_H)
    body += rect(WIN_X, WIN_Y, WIN_W, WIN_H, 11, fill="none", stroke=theme["border"], sw=1.0)
    return page(theme, window_shell(theme, body), caption, "版式 B · Windows 原生菜单栏")


def mock_menu_macos(theme, caption):
    preview = read_png("preview.png")
    y = WIN_Y
    body = titlebar_macos(theme, y, "preview.png")
    tool_y = y + TITLEBAR_H
    body += toolbar(theme, tool_y, variant="B")
    body += canvas(theme, tool_y + TOOLBAR_H, CANVAS_BOTTOM, preview)
    body += strip(theme, CANVAS_BOTTOM)
    body += status(theme, CANVAS_BOTTOM + STRIP_H)
    body += rect(WIN_X, WIN_Y, WIN_W, WIN_H, 11, fill="none", stroke=theme["border"], sw=1.0)
    return page(theme, window_shell(theme, body), caption, "版式 B · macOS 系统菜单栏", top=macos_system_bar(theme))


# ---------------------------------------------------------------- 规格页

def swatch(x, y, color, label, hex_value):
    out = rect(x, y, 96, 56, 8, fill=color, stroke="rgba(0,0,0,0.12)", sw=1)
    out += text(x, y + 74, label, size=11.5, fill="#1D1D1F", weight=500)
    out += text(x, y + 90, hex_value, size=11.0, fill="#8A8A90", spacing="0.4")
    return out


def section(x, y, number, title, note=""):
    out = text(x, y, number, size=12, fill="#B4B4BA", weight=600, spacing="0.6")
    out += text(x + 34, y, title, size=15, fill="#111114", weight=600)
    if note:
        out += text(x + 34 + len(title) * 15 + 12, y, note, size=11.5, fill="#8A8A90")
    return out


_MW_SEQ = [0]


def mini_window(x, y, w, h, theme, body):
    _MW_SEQ[0] += 1
    clip_id = f"mwclip{_MW_SEQ[0]}"
    out = rect(x + 1, y + 6, w, h, 10, fill="#000000", opacity=0.10)
    out += rect(x, y, w, h, 10, fill=theme["chrome"])
    out += f'<clipPath id="{clip_id}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="10"/></clipPath>'
    out += f'<g clip-path="url(#{clip_id})">{body}</g>'
    out += rect(x, y, w, h, 10, fill="none", stroke=theme["border"], sw=1)
    return out


def spec_page(theme):
    out = f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{PAGE_W}" height="1160" viewBox="0 0 {PAGE_W} 1160">'
    out += rect(0, 0, PAGE_W, 1160, 0, fill="#FBFBFC")
    out += text(96, 92, "image-view  ·  界面改版规格", size=29, fill="#111114", weight=600)
    out += text(96, 122, "macOS 视觉语言：中性灰底、极细描边、6 px 圆角、克制的强调色；外壳交给系统原生，三端一致。", size=13.5, fill="#6E6E73")
    out += line(96, 146, PAGE_W - 96, 146, "#ECECEF")

    # 01 色板
    out += section(96, 192, "01", "色板", "浅色为默认，深色跟随系统")
    light_row = [("#F6F6F7", "窗口 / 工具栏"), ("#E4E4E7", "分割线"), ("#FFFFFF", "控件底"),
                 ("#1C1C1E", "画布"), ("#1D1D1F", "主文字"), ("#6E6E73", "次文字"),
                 ("#0A7CFF", "强调"), ("#E5484D", "危险")]
    x = 96
    for color, label in light_row:
        out += swatch(x, 216, color, label, color)
        x += 116
    out += text(96, 338, "深色", size=12, fill="#6E6E73", weight=500)
    dark_row = [("#2A2A2D", "窗口 / 工具栏"), ("#3A3A3D", "分割线"), ("#3A3A3D", "控件底"),
                ("#101012", "画布"), ("#F5F5F7", "主文字"), ("#A1A1A6", "次文字"),
                ("#0A84FF", "强调"), ("#FF6961", "危险")]
    x = 96
    for color, label in dark_row:
        out += swatch(x, 350, color, label, color)
        x += 116

    # 02 布局与尺寸（按真实高度等比示意）
    out += section(96, 502, "02", "布局与尺寸")
    sx, sy = 96, 530
    out += mini_window(sx, sy, 460, 312, theme, "")
    out += rect(sx, sy + 30, 460, 52, 0, fill=theme["chrome"])
    out += line(sx, sy + 82, sx + 460, sy + 82, theme["border"], 1)
    out += text(sx + 14, sy + 20, "原生标题栏（系统绘制）", size=10.5, fill="#8A8A90")
    out += text(sx + 14, sy + 60, "工具栏 52", size=10.5, fill="#4A4A4F", weight=500)
    out += rect(sx, sy + 82, 460, 120, 0, fill="#1C1C1E")
    out += text(sx + 230, sy + 146, "画布（固定深色，图片优先）", size=11, fill="rgba(255,255,255,0.55)", anchor="middle")
    out += rect(sx, sy + 202, 460, 88, 0, fill="#EFEFF1")
    out += line(sx, sy + 202, sx + 460, sy + 202, theme["border"], 1)
    out += text(sx + 14, sy + 236, "缩略图条 88", size=10.5, fill="#4A4A4F", weight=500)
    for i in range(6):
        out += rect(sx + 132 + i * 46, sy + 224, 38, 26, 4, fill="#E3E3E7")
    out += rect(sx + 132 + 9 * 46 - 46, sy + 221, 44, 32, 6, fill="none", stroke=LIGHT["accent"], sw=1.5)
    out += rect(sx, sy + 290, 460, 22, 0, fill=theme["chrome"])
    out += line(sx, sy + 290, sx + 460, sy + 290, theme["border"], 1)
    out += text(sx + 14, sy + 305, "状态栏 26", size=10.5, fill="#4A4A4F", weight=500)

    specs = [
        ("工具栏高度", "52 px"),
        ("工具栏按钮", "30 × 30 · 圆角 6"),
        ("图标", "17 px · 线宽 1.5"),
        ("缩放分段控件", "118 × 30 · 圆角 6"),
        ("工具栏分组间距", "分隔线两侧各 12 px"),
        ("缩略图卡片", "76 × 56 · 圆角 6 · 间距 9"),
        ("状态栏 / 字号", "26 px · 11 px 次文字色"),
        ("画布内边距", "图片四周留 5 % 呼吸空间"),
    ]
    ty = sy + 8
    for name, value in specs:
        out += text(606, ty + 10, name, size=12, fill="#6E6E73")
        out += text(876, ty + 10, value, size=12, fill="#111114", weight=500, anchor="end")
        out += line(606, ty + 22, 876, ty + 22, "#F0F0F3")
        ty += 36

    # 03 图标
    out += section(96, 886, "03", "图标", "全部矢量绘制，随主题与状态着色")
    names = ["folder", "chevron-left", "chevron-right", "minus", "plus", "play", "sidebar",
             "sort", "ellipsis", "reveal", "copy", "trash", "help", "sliders"]
    labels = ["打开", "上一张", "下一张", "缩小", "放大", "幻灯片", "缩略图", "排序",
              "更多", "定位", "复制", "删除", "帮助", "设置"]
    x = 96
    for name, label in zip(names, labels):
        out += rect(x, 908, 64, 56, 8, fill="#F6F6F7", stroke="#ECECEF")
        out += icon(name, x + 24, 928, "#48484C", 16, 1.5)
        out += text(x + 32, 982, label, size=10.5, fill="#6E6E73", anchor="middle")
        x += 74

    # 04 控件状态
    out += section(96, 1040, "04", "控件状态")
    states = [("默认", "default", None), ("悬停", "hover", None), ("按下", "press", None),
              ("选中", "on", LIGHT["accent"]), ("禁用", "default", LIGHT["text3"])]
    x = 96
    for label, state, tint in states:
        out += rect(x, 1062, 60, 40, 8, fill="#F6F6F7", stroke="#ECECEF")
        out += icon_button(x + 15, 1067, "sidebar" if label == "选中" else "folder", LIGHT,
                           state=state, tint=tint)
        out += text(x + 30, 1120, label, size=10.5, fill="#6E6E73", anchor="middle")
        x += 74

    out += rect(x + 6, 1062, 190, 40, 8, fill="#F6F6F7", stroke="#ECECEF")
    out += segmented_zoom(x + 42, 1067, "46%", LIGHT)
    out += text(x + 100, 1120, "缩放（± 悬停时单独高亮）", size=10.5, fill="#6E6E73", anchor="middle")
    return out + "</svg>"


def spec_page2(theme):
    out = f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{PAGE_W}" height="1330" viewBox="0 0 {PAGE_W} 1330">'
    out += rect(0, 0, PAGE_W, 1330, 0, fill="#FBFBFC")
    out += text(96, 92, "image-view  ·  状态与三端", size=29, fill="#111114", weight=600)
    out += text(96, 122, "空状态、全屏、错误提示与平台差异。所有状态共用同一套控件与间距。", size=13.5, fill="#6E6E73")
    out += line(96, 146, PAGE_W - 96, 146, "#ECECEF")

    # 05 空状态
    out += section(96, 190, "05", "空状态", "首次启动、目录为空或关闭全部图片时")
    body = ""
    body += rect(96, 202, 560, 40, 0, fill=theme["chrome"])
    for i, color in enumerate(("#FF5F57", "#FEBC2E", "#28C840")):
        body += circle(116 + i * 20, 221, 6, fill=color)
    body += text(376, 226, "图片查看器", size=11.5, fill=LIGHT["text2"], anchor="middle")
    body += rect(96, 242, 560, 40, 0, fill=theme["chrome"])
    body += line(96, 282, 656, 282, theme["border"], 1)
    body += icon_button(108, 247, "folder", LIGHT)
    body += icon_button(148, 247, "chevron-left", LIGHT, tint=LIGHT["text3"])
    body += icon_button(180, 247, "chevron-right", LIGHT, tint=LIGHT["text3"])
    body += rect(96, 282, 560, 282, 0, fill="#1C1C1E")
    body += line(96, 564, 656, 564, theme["border"], 1)
    body += rect(96, 564, 560, 26, 0, fill=theme["chrome"])
    body += text(110, 581, "未打开图片", size=11, fill="#9A9AA0")
    out += mini_window(96, 202, 560, 388, theme, body)
    # 空状态内容
    cx = 376
    out += rect(cx - 30, 336, 60, 60, 15, fill="rgba(255,255,255,0.06)", stroke="rgba(255,255,255,0.12)", sw=1)
    out += icon("sidebar", cx - 13, 354, "rgba(255,255,255,0.5)", 26, 1.4)
    out += text(cx, 424, "还没有打开图片", size=15, fill="rgba(255,255,255,0.85)", weight=500, anchor="middle")
    out += text(cx, 446, "拖入文件，或按 Ctrl+O 选择", size=12, fill="rgba(255,255,255,0.45)", anchor="middle")
    out += rect(cx - 62, 470, 124, 32, 7, fill=LIGHT["accent"])
    out += text(cx, 490.5, "打开图片…", size=12.5, fill="#FFFFFF", weight=500, anchor="middle")

    notes = [
        ("图标 + 一句话说明 + 主按钮", "空状态承担“打开”的首要入口：中心图标 60×60，标题 15 px，说明 12 px 弱化，主按钮为强调色实心。"),
        ("工具栏降级为禁用", "无图时前后翻页、缩放、幻灯片全部置灰，只保留“打开”可用，避免误操作。"),
        ("缩略图条隐藏", "目录为空时整条收起，画布占满剩余高度，窗口不会出现空条。"),
    ]
    ny = 250
    for title, desc in notes:
        out += text(700, ny, "· " + title, size=12.5, fill="#111114", weight=600)
        out += text(700, ny + 22, desc, size=11.5, fill="#6E6E73")
        ny += 76

    # 06 全屏
    out += section(96, 650, "06", "全屏", "F11 / 双击进入；系统标题栏隐藏后由工具栏中央接管文件名")
    body = rect(96, 674, 560, 40, 0, fill=theme["chrome"])
    body += text(376, 698, "preview.png  ·  9 / 10", size=12.5, fill=LIGHT["text"], weight=500, anchor="middle")
    body += icon_button(108, 679, "chevron-left", LIGHT)
    body += icon_button(140, 679, "chevron-right", LIGHT)
    body += rect(96, 714, 560, 120, 0, fill="#1C1C1E")
    body += rect(126, 722, 500, 104, 2, fill="#FFFFFF")
    body += rect(148, 736, 456, 60, 2, fill="#F2F5FA")
    body += text(376, 770, "图片", size=10, fill="#B4B4BA", anchor="middle")
    out += mini_window(96, 674, 560, 160, theme, body)
    out += text(700, 700, "· 标题回到工具栏中央", size=12.5, fill="#111114", weight=600)
    out += text(700, 722, "全屏时无系统标题栏，工具栏中央显示“文件名 · 序号”，3 秒无操作后自动淡出（移动鼠标唤回）。", size=11.5, fill="#6E6E73")

    # 07 错误与提示
    out += section(96, 860, "07", "错误与短提示")
    body = rect(96, 872, 560, 34, 0, fill=theme["chrome"])
    for i, color in enumerate(("#FF5F57", "#FEBC2E", "#28C840")):
        body += circle(116 + i * 20, 889, 5.5, fill=color)
    body += text(376, 894, "损坏的文件.jpg", size=11.5, fill=LIGHT["text2"], anchor="middle")
    body += rect(96, 906, 560, 40, 0, fill=theme["chrome"])
    body += line(96, 946, 656, 946, theme["border"], 1)
    body += icon_button(108, 911, "folder", LIGHT)
    body += icon_button(146, 911, "chevron-left", LIGHT, tint=LIGHT["text3"])
    body += icon_button(178, 911, "chevron-right", LIGHT, tint=LIGHT["text3"])
    body += rect(96, 946, 560, 34, 0, fill="#FDECEC")
    body += rect(96, 946, 3, 34, 0, fill=LIGHT["danger"])
    body += icon("warning", 112, 956, LIGHT["danger"], 14, 1.5)
    body += text(134, 967, "无法解码该文件：不支持的色彩配置", size=11.5, fill="#8C2F32")
    body += text(614, 967, "重试", size=11.5, fill=LIGHT["danger"], weight=500, anchor="end")
    body += rect(96, 980, 560, 84, 0, fill="#1C1C1E")
    body += text(376, 1026, "（画布保持上一张图片）", size=11, fill="rgba(255,255,255,0.35)", anchor="middle")
    body += rect(96, 1064, 560, 26, 0, fill=theme["chrome"])
    body += text(110, 1081, "已复制路径", size=11, fill="#2F8F4E")
    out += mini_window(96, 872, 560, 218, theme, body)
    out += text(700, 910, "· 错误：内联横幅", size=12.5, fill="#111114", weight=600)
    out += text(700, 932, "不再用整块深红横条；改为浅红底 + 左侧 3 px 竖条 + 行内“重试”，画布内容不被打断。", size=11.5, fill="#6E6E73")
    out += text(700, 1000, "· 短提示：状态栏右侧", size=12.5, fill="#111114", weight=600)
    out += text(700, 1022, "复制路径、删除成功等一次性反馈出现在状态栏右侧，4 秒后淡出，不弹窗、不打断浏览。", size=11.5, fill="#6E6E73")

    # 08 三端差异
    out += section(96, 1140, "08", "三端差异", "同一套视觉，外壳交给系统")
    rows = [
        ("macOS", "系统菜单栏 + 红黄绿窗口按钮",
         "文件 / 编辑 / 显示 / 帮助 全部走屏幕顶部的系统菜单；工具栏不再放“更多”入口，符合 mac 习惯。"),
        ("Windows", "默认工具栏；可选原生 Win32 菜单栏",
         "开启后菜单条位于标题栏下方，浅色/深色随主题切换；关闭时全部入口收在工具栏与右键菜单里。"),
        ("Linux", "工具栏 + 右键菜单",
         "没有可靠的跨桌面原生菜单条，统一用工具栏；快捷键、图标与三端完全一致。"),
    ]
    out += text(96, 1178, "平台", size=11, fill="#9A9AA0")
    out += text(190, 1178, "菜单与外壳", size=11, fill="#9A9AA0")
    out += text(560, 1178, "说明", size=11, fill="#9A9AA0")
    for i, (platform, shell, desc) in enumerate(rows):
        ry = 1196 + i * 44
        out += text(96, ry + 14, platform, size=12.5, fill="#111114", weight=600)
        out += text(190, ry + 14, shell, size=11.5, fill="#4A4A4F")
        out += text(560, ry + 14, desc, size=11.5, fill="#6E6E73")
        out += line(96, ry + 26, PAGE_W - 96, ry + 26, "#F0F0F3")
    return out + "</svg>"


# ---------------------------------------------------------------- 输出

def render(svg: str, name: str, width: int = PAGE_W, view_box: str | None = None) -> None:
    if view_box:
        head_end = svg.index(">") + 1
        head = svg[:head_end]
        import re

        head = re.sub(r'width="[^"]*"', 'width="800"', head, count=1)
        head = re.sub(r'viewBox="[^"]*"', f'viewBox="{view_box}"', head, count=1)
        svg = head + svg[head_end:]
    data = resvg_py.svg_to_bytes(svg_string=svg, width=width)
    out = os.path.join(HERE, name)
    with open(out, "wb") as handle:
        handle.write(bytes(data))
    print(f"  -> {name}")


def main() -> None:
    os.makedirs(HERE, exist_ok=True)
    light = mock_toolbar(LIGHT, "preview.png — 图片查看器", "image-view · 界面改版")
    render(light, "01-toolbar-light.png")
    render(mock_toolbar(DARK, "preview.png — 图片查看器", "image-view · 界面改版 · 深色跟随系统"), "02-toolbar-dark.png")
    render(mock_menu_windows(LIGHT, "image-view · 界面改版"), "03-menu-windows.png")
    render(mock_menu_macos(LIGHT, "image-view · 界面改版"), "04-menu-macos.png")
    render(mock_toolbar_windows(LIGHT, "image-view · 界面改版"), "07-toolbar-windows.png")
    render(spec_page(LIGHT), "05-spec.png")
    render(spec_page2(LIGHT), "06-spec-states.png")
    if os.environ.get("CROPS"):
        inner = body_toolbar(LIGHT, "preview.png — 图片查看器")
        closeup(LIGHT, WIN_X, WIN_Y + TITLEBAR_H - 6, WIN_W, 70, inner, "crop-toolbar.png")
        closeup(LIGHT, WIN_X + 380, CANVAS_BOTTOM - 6, 700, 110, inner, "crop-strip.png")


if __name__ == "__main__":
    main()
