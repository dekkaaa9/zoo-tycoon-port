r"""Build the launcher icons from the original game's own icon.

The only icon Zoo Tycoon ships is the one embedded in `zoo.exe` as PE resources: an
`RT_GROUP_ICON` directory (width, height, planes, bit count and the id of each image) plus
the `RT_ICON` images themselves, and it is 32x32 at 8 bits. This script pulls that out and
writes every launcher density under `app/src/main/res/`.

    python make-launcher-icons.py "D:\Zoo Tycoon"

Two things about the PE format are easy to get wrong and are worth stating:

* a resource directory entry is **8 bytes**, not 16 (the 16-byte one is the *data* entry it
  points at), and both the subdirectory pointer and the data-entry pointer are offsets from
  the start of the resource section, not from the directory that holds them;
* `SizeOfOptionalHeader` lives in the COFF header, four bytes before the optional header.

The upscale is a supersample: nearest neighbour to four times the target, then Lanczos back
down. A plain Lanczos of a 32 px source is mush, and a plain nearest blow-up is blocks; this
keeps the lettering of the "ZOO" logo crisp without the stair-stepping.
"""

import os
import struct
import sys

import numpy as np
from PIL import Image, ImageFilter

RT_ICON = 3
RT_GROUP_ICON = 14

DENSITIES = [
    ("mipmap-mdpi", 48),
    ("mipmap-hdpi", 72),
    ("mipmap-xhdpi", 96),
    ("mipmap-xxhdpi", 144),
    ("mipmap-xxxhdpi", 192),
]

SUPERSAMPLE = 4


def u16(data, off):
    return struct.unpack_from("<H", data, off)[0]


def u32(data, off):
    return struct.unpack_from("<I", data, off)[0]


def rva_to_offset(rva, sections):
    for _, va, vsize, raw, rsize in sections:
        if va <= rva < va + max(vsize, rsize):
            return raw + (rva - va)
    return None


def parse_sections(data, opt_off, n_sections):
    # SizeOfOptionalHeader is in the COFF header, four bytes before the optional one.
    sections = []
    off = opt_off + u16(data, opt_off - 4)
    for _ in range(n_sections):
        vsize, va, rsize, raw = struct.unpack_from("<IIII", data, off + 8)
        sections.append((data[off:off + 8].rstrip(b"\0"), va, vsize, raw, rsize))
        off += 40
    return sections


def walk(data, root, base, sections, path=()):
    """Yield (type, name, rva, size) for every resource leaf."""
    for i in range(u16(data, base + 12) + u16(data, base + 14)):
        entry = base + 16 + 8 * i
        name_field = u32(data, entry)
        offset = u32(data, entry + 4)
        is_dir = bool(offset & 0x80000000)
        offset &= 0x7FFFFFFF
        key = ("name", name_field) if i < u16(data, base + 12) else ("id", name_field)
        if is_dir:
            yield from walk(data, root, root + offset, sections, path + (key,))
        else:
            data_entry = root + offset
            yield path + (key,), u32(data, data_entry), u32(data, data_entry + 4)


def read_icon(exe):
    """The largest group icon of `exe`, as a list of (width, height, bits, bytes)."""
    data = open(exe, "rb").read()
    assert data[:2] == b"MZ", "not a PE file"
    pe = u32(data, 0x3C)
    assert data[pe:pe + 4] == b"PE\0\0", "not a PE file"
    n_sections = u16(data, pe + 6)
    opt_off = pe + 24
    magic = u16(data, opt_off)
    dirs = opt_off + (112 if magic == 0x20B else 96)
    # Data directory index 2 is the resource table.
    res_rva = u32(data, dirs + 16)

    sections = parse_sections(data, opt_off, n_sections)
    res_base = rva_to_offset(res_rva, sections)
    assert res_base is not None, "no resource section"

    groups, icons = {}, {}
    for path, rva, size in walk(data, res_base, res_base, sections):
        kind, name = path[0][1], path[1][1]
        if kind == RT_GROUP_ICON:
            groups[name] = rva
        elif kind == RT_ICON:
            icons[name] = rva

    assert groups, "no RT_GROUP_ICON in %s" % exe

    best = None
    for rva in groups.values():
        off = rva_to_offset(rva, sections)
        count = u16(data, off + 4)
        entries = [struct.unpack_from("<BBBBHHII", data, off + 6 + 14 * i)
                   for i in range(count)]
        entries = [e for e in entries if e[7] in icons]
        if not entries:
            continue
        top = max(entries, key=lambda e: (e[0] * e[1], e[3]))
        if best is None or (top[0] * top[1], top[3]) > (best[0][0] * best[0][1], best[0][3]):
            best = (top, entries)

    top, entries = best
    w, h, colours, _, planes, bits, nbytes, image_id = top
    img_off = rva_to_offset(icons[image_id], sections)
    # The group icon's ColorCount is 0 for an 8-bit image, which means the full 256; the
    # DIB's own biClrUsed is what the decoder reads.
    print("%s: group icon %dx%d, %d bits, %d bytes"
          % (os.path.basename(exe), w, h, bits, nbytes))
    return w, h, bits, data[img_off:img_off + nbytes]


def decode(w, h, bits, image):
    """The DIB of an icon image: palette, bottom-up XOR bitmap, bottom-up AND mask."""
    size, width, height, _, depth, _, _, _, _, used, _ = \
        struct.unpack_from("<IiiHHIIiiII", image, 0)
    assert (size, depth) == (40, bits), "unexpected DIB header"
    rows = height // 2
    palette = np.frombuffer(image, np.uint8, used * 4, 40).reshape(used, 4)[:, :3].copy()

    xor_at = 40 + used * 4
    xor = np.frombuffer(image, np.uint8, rows * width, xor_at).reshape(rows, width)
    and_at = xor_at + rows * width
    and_rows = (width + 31) // 32 * 4
    mask = np.frombuffer(image, np.uint8, and_rows * rows, and_at).reshape(rows, and_rows)

    rgb = palette[xor[::-1]]
    alpha = np.zeros((rows, width), np.uint8)
    for row in range(rows):
        packed = np.unpackbits(mask[rows - 1 - row], bitorder="little")[:width]
        alpha[row] = np.where(packed == 0, 255, 0)
    return Image.fromarray(np.dstack([rgb, alpha]), "RGBA")


def main():
    game = sys.argv[1] if len(sys.argv) > 1 else r"D:\Zoo Tycoon"
    here = os.path.dirname(os.path.abspath(__file__))
    res = os.path.join(here, "app", "src", "main", "res")

    w, h, bits, image = read_icon(os.path.join(game, "zoo.exe"))
    src = decode(w, h, bits, image)
    print("decoded %dx%d, %d opaque of %d pixels"
          % (src.size[0], src.size[1],
             int((np.asarray(src)[:, :, 3] > 200).sum()), w * h))

    for name, px in DENSITIES:
        big = src.resize((px * SUPERSAMPLE,) * 2, Image.NEAREST)
        big = big.resize((px, px), Image.LANCZOS)
        big = big.filter(ImageFilter.UnsharpMask(radius=max(1, px // 48),
                                                 percent=80, threshold=0))
        out = os.path.join(res, name, "ic_launcher.png")
        big.save(out)
        print("  %-18s %dx%d -> %s" % (name, px, px, out))


if __name__ == "__main__":
    main()
