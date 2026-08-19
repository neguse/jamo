#!/usr/bin/env python3
"""jamo 辞書バイナリ (v2) のセクション別サイズ内訳を表示する。

使い方: scripts/dict-breakdown.py data/mozc.dict

doc/dict-format.md の v2 レイアウトに準拠してパースする。
`jamo dict info` (T140) が実装されたらそちらに置き換える。
"""

import struct
import sys


def main(path):
    with open(path, "rb") as f:
        data = f.read()

    off = 0

    def u32():
        nonlocal off
        v = struct.unpack_from("<I", data, off)[0]
        off += 4
        return v

    if data[0:4] != b"JAMO":
        sys.exit(f"{path}: bad magic")
    off = 4
    version = u32()

    bit_len = u32()
    block_count = u32()
    louds_bytes = 8 + block_count * 8
    off += block_count * 8

    label_count = u32()
    labels_bytes = 4 + label_count
    off += label_count

    terminal_count = u32()
    terminal_bytes = 4 + terminal_count * 8
    sentinel = 0
    for i in range(terminal_count):
        if struct.unpack_from("<I", data, off + i * 8)[0] == 0xFFFFFFFF:
            sentinel += 1
    off += terminal_count * 8

    entry_count = u32()
    entries_start = off
    surface_total = 0
    surfaces = set()
    for _ in range(entry_count):
        slen = struct.unpack_from("<H", data, off)[0]
        surfaces.add(data[off + 2:off + 2 + slen])
        surface_total += slen
        off += 2 + slen + 7
    entries_bytes = 4 + (off - entries_start)

    num_pos = u32()
    matrix_bytes = 4 + num_pos * num_pos * 2
    off += num_pos * num_pos * 2

    vocab = u32()
    readings = u32()

    total = len(data)
    uniq_bytes = sum(len(s) for s in surfaces)

    def row(name, nbytes, note=""):
        print(f"{name:<16} {nbytes / 1e6:8.1f} MB  {nbytes / total:6.1%}  {note}")

    print(f"{path} (v{version}, total {total / 1e6:.1f} MB)")
    row("louds bitvec", louds_bytes, f"{bit_len} bits")
    row("labels", labels_bytes, f"{label_count} nodes")
    row("terminal", terminal_bytes,
        f"{terminal_count} nodes, sentinel {sentinel / terminal_count:.1%}")
    row("entries", entries_bytes, f"{entry_count} entries")
    row("  surface", surface_total,
        f"uniq {len(surfaces)} ({uniq_bytes / 1e6:.1f} MB)")
    row("  fixed 9B", entry_count * 9, "left/right/cost/pos_type + len")
    row("conn matrix", matrix_bytes, f"num_pos {num_pos}")
    print(f"stats: vocab {vocab}, readings {readings}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
