"""生成 Windows 安装包与 exe 用的多尺寸 ICO。

用法（仓库根目录）：
    python design/gen_ico.py

输入 design/icon-1024.png，输出 assets/icon.ico（16/24/32/48/64/128/256）。
再用 llvm-rc 编译 assets/app.rc → assets/app.res，供 build.rs 链接进 exe。
"""

from pathlib import Path

from PIL import Image

SIZES = (16, 24, 32, 48, 64, 128, 256)
ROOT = Path(__file__).resolve().parent.parent


def main() -> None:
    source = ROOT / "design" / "icon-1024.png"
    if not source.is_file():
        raise SystemExit(f"missing source icon: {source}")

    master = Image.open(source).convert("RGBA")
    output = ROOT / "assets" / "icon.ico"
    output.parent.mkdir(parents=True, exist_ok=True)
    master.save(output, format="ICO", sizes=[(size, size) for size in SIZES])

    print(f"wrote {output} ({output.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
