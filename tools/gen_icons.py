#!/usr/bin/env python3
"""生成 Word Collector 全套图标（无第三方依赖，纯 Python PNG 编码）。

产物：
  src-tauri/icons/32x32.png, 128x128.png, 128x128@2x.png, icon.png
  src-tauri/icons/icon.icns   (iconutil 从 iconset 生成)
  src-tauri/icons/icon.ico    (PNG 压缩条目, Vista+ 支持)
  src-tauri/icons/tray.png    (托盘/菜单栏图标 64px)
"""
import os
import struct
import subprocess
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
ICON_DIR = os.path.join(HERE, "..", "src-tauri", "icons")
SS = 2  # 超采样倍率（抗锯齿）

# Obsidian 风格紫罗兰渐变
TOP = (0x8B, 0x6F, 0xE8)
BOTTOM = (0x4C, 0x2A, 0xA8)


def png_chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def encode_png(width: int, height: int, rgba: bytes) -> bytes:
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    rows = bytearray()
    stride = width * 4
    for y in range(height):
        rows.append(0)  # filter: None
        rows += rgba[y * stride : (y + 1) * stride]
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", ihdr)
        + png_chunk(b"IDAT", zlib.compress(bytes(rows), 9))
        + png_chunk(b"IEND", b"")
    )


def inside_rounded_rect(u, v, x0, y0, x1, y1, r):
    if u < x0 or u > x1 or v < y0 or v > y1:
        return False
    cx = min(max(u, x0 + r), x1 - r)
    cy = min(max(v, y0 + r), y1 - r)
    return (u - cx) ** 2 + (v - cy) ** 2 <= r * r


def in_bookmark(u, v):
    """白色书签图形：圆角矩形 + 底部三角缺口。"""
    if not inside_rounded_rect(u, v, 0.37, 0.22, 0.63, 0.80, 0.03):
        return False
    # 三角缺口：从底边向上收
    t = (0.80 - v) / (0.80 - 0.60)
    half = 0.13 * max(0.0, t)
    return not (0.50 - half <= u <= 0.50 + half and v >= 0.60)


def sample(u, v):
    """单个归一化采样点 → (r, g, b, a)。"""
    if not inside_rounded_rect(u, v, 0.04, 0.04, 0.96, 0.96, 0.20):
        return (0, 0, 0, 0)
    t = v
    r = int(TOP[0] + (BOTTOM[0] - TOP[0]) * t)
    g = int(TOP[1] + (BOTTOM[1] - TOP[1]) * t)
    b = int(TOP[2] + (BOTTOM[2] - TOP[2]) * t)
    if in_bookmark(u, v):
        return (255, 255, 255, 255)
    return (r, g, b, 255)


def render_master(size=1024):
    """超采样渲染 1024px 主图，输出 (size, size, rgba bytes)。"""
    grid = size * SS
    rows = []
    for gy in range(grid):
        v = (gy + 0.5) / grid
        row = bytearray()
        for gx in range(grid):
            u = (gx + 0.5) / grid
            r, g, b, a = sample(u, v)
            row += bytes((r, g, b, a))
        rows.append(bytes(row))
    rgba = b"".join(rows)
    # 盒式下采样 SS×SS
    out = bytearray(size * size * 4)
    for y in range(size):
        for x in range(size):
            r = g = b = a = 0
            base = (y * grid + x * SS) * 4
            for dy in range(SS):
                src = base + dy * grid * 4
                for dx in range(SS):
                    i = src + dx * 4
                    r += rgba[i]
                    g += rgba[i + 1]
                    b += rgba[i + 2]
                    a += rgba[i + 3]
            div = SS * SS
            o = (y * size + x) * 4
            out[o] = r // div
            out[o + 1] = g // div
            out[o + 2] = b // div
            out[o + 3] = a // div
    return size, bytes(out)


def downsample(master_rgba, master_size, target):
    """从主图盒式下采样到 target（要求 target 整除 master_size）。"""
    factor = master_size // target
    out = bytearray(target * target * 4)
    for y in range(target):
        for x in range(target):
            r = g = b = a = 0
            for dy in range(factor):
                src = ((y * factor + dy) * master_size + x * factor) * 4
                for dx in range(factor):
                    i = src + dx * 4
                    r += master_rgba[i]
                    g += master_rgba[i + 1]
                    b += master_rgba[i + 2]
                    a += master_rgba[i + 3]
            div = factor * factor
            o = (y * target + x) * 4
            out[o] = r // div
            out[o + 1] = g // div
            out[o + 2] = b // div
            out[o + 3] = a // div
    return bytes(out)


def write_ico(path, entries):
    """entries: [(size, png_bytes)]，写 PNG 压缩条目的 ICO。"""
    header = struct.pack("<HHH", 0, 1, len(entries))
    body = b""
    offset = 6 + 16 * len(entries)
    directory = b""
    for size, png in entries:
        w = 0 if size >= 256 else size
        directory += struct.pack(
            "<BBBBHHII", w, w, 0, 0, 1, 32, len(png), offset
        )
        body += png
        offset += len(png)
    with open(path, "wb") as f:
        f.write(header + directory + body)


def main():
    os.makedirs(ICON_DIR, exist_ok=True)
    print("渲染 1024px 主图（超采样 x%d）…" % SS)
    size, master = render_master(1024)
    print("主图完成，开始派生各尺寸…")

    def png_at(target):
        return encode_png(target, target, downsample(master, size, target))

    # Tauri 需要的 PNG
    for target, name in [
        (32, "32x32.png"),
        (128, "128x128.png"),
        (256, "128x128@2x.png"),
        (512, "icon.png"),
        (64, "tray.png"),
    ]:
        path = os.path.join(ICON_DIR, name)
        with open(path, "wb") as f:
            f.write(png_at(target))
        print("  写入", path)

    # macOS iconset → icns
    iconset = os.path.join(ICON_DIR, "icon.iconset")
    os.makedirs(iconset, exist_ok=True)
    for target, name in [
        (16, "icon_16x16.png"),
        (32, "icon_16x16@2x.png"),
        (32, "icon_32x32.png"),
        (64, "icon_32x32@2x.png"),
        (128, "icon_128x128.png"),
        (256, "icon_128x128@2x.png"),
        (256, "icon_256x256.png"),
        (512, "icon_256x256@2x.png"),
        (512, "icon_512x512.png"),
        (1024, "icon_512x512@2x.png"),
    ]:
        with open(os.path.join(iconset, name), "wb") as f:
            f.write(png_at(target))
    icns = os.path.join(ICON_DIR, "icon.icns")
    subprocess.run(
        ["iconutil", "-c", "icns", iconset, "-o", icns], check=True
    )
    print("  写入", icns)

    # Windows ico（PNG 条目）
    ico = os.path.join(ICON_DIR, "icon.ico")
    write_ico(ico, [(s, png_at(s)) for s in (16, 24, 32, 48, 64, 128, 256)])
    print("  写入", ico)

    print("全部图标生成完毕。")


if __name__ == "__main__":
    sys.exit(main())
