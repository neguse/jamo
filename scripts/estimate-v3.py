#!/usr/bin/env python3
"""v3 辞書フォーマット各施策のサイズを mozc 実データで見積もる (T141 設計根拠)。

使い方: scripts/fetch-mozc.sh 実行済みの状態で scripts/estimate-v3.py

trie ノード数はソート済みキーの LCP 法 (nodes = 1 + Σ(len - lcp(prev)))
で厳密に数える。現行 v2 の実ノード数 (jamo dict info の labels) と一致する
ことで手法を検証できる。

TODO: v3 実装完了後は `jamo dict info` の実測に役目を譲って削除する。
"""
import collections
import glob


def kata_to_hira(s):
    return ''.join(chr(ord(c) - 0x60) if 0x30A1 <= ord(c) <= 0x30F6 else c for c in s)


def trie_nodes(sorted_keys):
    total = 1  # root
    prev = b''
    for k in sorted_keys:
        lcp = 0
        for a, b in zip(prev, k):
            if a != b:
                break
            lcp += 1
        total += len(k) - lcp
        prev = k
    return total


def varint_len(v):
    n = 1
    while v >= 0x80:
        v >>= 7
        n += 1
    return n


def main():
    entries = []
    for path in sorted(glob.glob('data/mozc/dictionary*.txt')):
        for line in open(path, encoding='utf-8'):
            p = line.rstrip('\n').split('\t')
            if len(p) < 5:
                continue
            entries.append((kata_to_hira(p[0]), int(p[1]), int(p[2]), int(p[3]), p[4]))
    readings = sorted(set(e[0] for e in entries))
    print(f"entries {len(entries)}, readings {len(readings)}")

    # --- 読み trie (T148: 1バイト符号化) ---
    chars = sorted({c for y in readings for c in y})
    print(f"reading charset: {len(chars)} (1バイト符号化可: {len(chars) <= 255})")
    utf8_nodes = trie_nodes([y.encode() for y in readings])
    code = {c: i for i, c in enumerate(chars)}
    enc_nodes = trie_nodes(sorted(bytes(code[c] for c in y) for y in readings))
    print(f"trie nodes: utf8 {utf8_nodes} -> 1-byte {enc_nodes} ({enc_nodes / utf8_nodes:.1%})")

    # trie 部: LOUDS bits (2n) + labels (n) + 終端フラグ (n/8)
    #        + エントリ境界 BitVec (entry_count bits、終端 k 番目の範囲を select で引く)
    trie_bytes = (2 * enc_nodes) // 8 + enc_nodes + enc_nodes // 8 + len(entries) // 8

    # --- token 部 (T146: 恒等表層フラグ、combo 辞書化) ---
    combos = set()
    ident = 0
    surf_pool = set()
    for y, _l, _r, _c, s in entries:
        combos.add((_l, _r))
        if s == y or kata_to_hira(s) == y:
            ident += 1
        else:
            surf_pool.add(s)
    non_ident = len(entries) - ident
    print(f"id combos {len(combos)}, identity {ident / len(entries):.1%}, "
          f"non-ident uniq surfaces {len(surf_pool)}")
    # entry: combo_id 14bit + 恒等フラグ 2bit + cost 16bit = 4B、非恒等のみ表層参照 3B
    token_bytes = len(entries) * 4 + non_ident * 3
    combo_table = len(combos) * 5  # left 2B + right 2B + pos_type 1B

    # --- 表層プール (T149: 可変長文字符号化 高頻度224字=1B、残り2B) ---
    surf_sorted = sorted(surf_pool)
    freq = collections.Counter()
    for s in surf_sorted:
        freq.update(s)
    top = set(c for c, _ in freq.most_common(224))
    pool_bytes = sum(sum(1 if c in top else 2 for c in s) for s in surf_sorted)
    pool_meta = len(surf_sorted)  # len prefix 1B
    code_table = len(freq) * 3  # 文字 -> UTF-8 復元表
    print(f"surface pool: utf8 {sum(len(s.encode()) for s in surf_sorted) / 1e6:.1f} MB "
          f"-> encoded {pool_bytes / 1e6:.1f} MB")

    # --- 接続行列 (T147: 行ごと最頻値デフォルト + 例外 varint) ---
    vals = open('data/mozc/connection_single_column.txt').read().split()
    n = int(vals[0])
    mat = [int(v) for v in vals[1:]]
    matrix_bytes = n * 6  # 行ヘッダ: default 2B + offset 4B
    exceptions = 0
    for r in range(n):
        row = mat[r * n:(r + 1) * n]
        default, _ = collections.Counter(row).most_common(1)[0]
        prev_col = 0
        for col, v in enumerate(row):
            if v != default:
                exceptions += 1
                delta = v - default
                zigzag = (delta << 1) ^ (delta >> 31) if delta >= 0 else ((-delta) << 1) - 1
                matrix_bytes += varint_len(col - prev_col) + varint_len(zigzag)
                prev_col = col
    print(f"matrix: {n}x{n}, exceptions {exceptions / (n * n):.1%}")

    # --- 合計 ---
    header = 128
    rows = [
        ("reading trie (1B enc)", trie_bytes),
        ("token (4B + 3B non-ident)", token_bytes),
        ("combo table", combo_table),
        ("surface pool (encoded)", pool_bytes + pool_meta + code_table),
        ("conn matrix (varint)", matrix_bytes),
        ("header", header),
    ]
    total = sum(b for _, b in rows)
    print()
    for name, b in rows:
        print(f"{name:<28} {b / 1e6:8.2f} MB")
    print(f"{'v3 TOTAL':<28} {total / 1e6:8.2f} MB  (目標 <= 20 MB、"
          f"本家 mozc_server 22.7 MB)")


if __name__ == "__main__":
    main()
