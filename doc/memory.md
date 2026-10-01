# メモリ使用量

かな漢字変換エンジンとしての実メモリ消費の記録。再計測:

```bash
cargo build --release -p jamo-cli
python3 scripts/bench-memory.py --with-anthy --with-mozc-server   # ext4 等のページキャッシュ
python3 scripts/bench-memory.py --shm                             # tmpfs (4KB ページ)
```

計測は fork + exec + wait4 だけの C ランチャー (スクリプトが `cc` でビルド) で
子プロセスの `ru_maxrss` を取る。Python から直接 fork すると exec 前の子が Python の
RSS (~10MB) を引き継ぎ、それが下限になって小さい値が測れない (旧計測の落とし穴)。

## 他エンジン比較 (2026-10-01, Linux x86_64 6.18, jamo v3)

| エンジン | 辞書データ | peak RSS | 計測条件 |
|---------|-----------|----------|---------|
| jamo + mozc.dict (v3) | 19.3 MB (.dict) | **17.9 MB** | 変換1回込み (mmap、ext4 ページキャッシュ) |
| jamo + mozc.dict (v3) | 19.3 MB (.dict) | **9.5 MB** | 変換1回込み (mmap、tmpfs 4KB ページ) |
| jamo + ipadic.dict (v3) | 8.4 MB (.dict) | **11.2 MB** | 変換1回込み (mmap、ext4) |
| jamo + ipadic.dict (v3) | 8.4 MB (.dict) | **7.0 MB** | 変換1回込み (mmap、tmpfs) |
| jamo + mozc.dict (v2、参考) | 83.5 MB (.dict) | 152.6 MB | 変換1回込み (全ヒープ展開) |
| jamo + ipadic.dict (v2、参考) | 20.1 MB (.dict) | 42.9 MB | 変換1回込み |
| Anthy (anthy-unicode 1.0.0.20260213) | 20.3 MB (anthy.dic) | 16.8 MB | 付属テスト約550件の変換込み (2026-08-19) |
| mozc_server (本家 fcitx5-mozc) | 22.7 MB (バイナリ埋め込み) | 6.2 MB | 起動直後・変換なし (下限値、2026-08-19) |

- jamo-cli 自体のベースライン (辞書を開かない usage 表示) は 3.3 MB。
- **ext4 と tmpfs で差が出る理由**: Linux 6.x のページキャッシュは large folio で、
  mmap した辞書に 1 バイト触れるだけで 2MB 単位がマップされる (`/proc/self/smaps` で
  確認: 1 バイト → Rss 2048 kB)。ファイル全体 19.3 MB のうちほぼ全域に 1 回は触れる
  ため、ext4 では辞書サイズ近くまで RSS が伸びる。tmpfs は 4KB ページ (fault-around
  64KB 単位) なので、実際に触れたページに近い値になる。組み込み (DM250 の SD など
  4KB ページ環境) のワーキングセットは tmpfs の値が目安。
- mozc_server はクライアントなしでは変換を実行できないため起動直後の値。この計測環境
  (コンテナ) ではデーモンが即終了して観測できないので 2026-08-19 の実測値を再掲。
  辞書をバイナリに埋め込み mmap + lazy load するため、変換後は 22.7 MB + 作業領域が
  上限の目安。
- 同じ mozc dictionary_oss ソースを、本家は 22.7 MB のバイナリ (辞書 + 接続行列の
  データ部は 16.87 MB)、jamo v3 は 19.3 MB の .dict に収める。バイナリ対バイナリでは
  jamo が小さく、データ対データでは 2.4 MB 及ばない (T151 の bit 符号化で追う)。

## 辞書バイナリ内訳 (フォーマット v3)

`cargo run -p jamo-cli --release -- dict info data/mozc.dict` (2026-10-01):

```
data/mozc.dict (v3, total 19.3 MB)
louds bitvec         0.80 MB    4.2%
labels               1.84 MB    9.5%
terminal bitvec      0.40 MB    2.1%
entry bounds         0.28 MB    1.5%
tokens               5.16 MB   26.8%
combos               0.06 MB    0.3%
nonident bitvec      0.28 MB    1.5%
surface ids          2.67 MB   13.9%
surface pool         3.99 MB   20.7%
surface bounds       0.87 MB    4.5%
conn matrix          2.89 MB   15.0%
trie nodes 1837020, tokens 1289076, id combos 11370, pool surfaces 698652,
matrix 2672x2672 (exceptions 884398)
```

v2 (83.5 MB、RSS 152 MB) からの削減の内訳 (doc/dict-format.md):

1. **ロード方式**: 全エントリの String/Vec ヒープ展開をやめ、mmap したバイト列を
   借用参照する。ロード 0.20 s → 0.1 ms
2. **terminal 35.1 MB → 0.7 MB**: 83% が非終端センチネルだった (start, end) 配列を
   終端フラグ BitVec + エントリ境界 BitVec (select で範囲) にした
3. **恒等表層 (31.7%)**: 読みから生成できる表層をフラグ化し格納しない
4. **表層プール 8.3 MB → 4.0 MB**: 可変長文字符号化 (上位 224 字 = 1 バイト)
5. **読み 1 バイト符号**: trie ノード 4.39M → 1.84M (41.9%)
6. **接続行列 14.3 MB → 2.9 MB**: 行最頻値デフォルト + 例外 BitVec + コスト配列。
   参照は rank 1 回で O(1) (varint 案はスキャンが要るので却下)

## 残る削減余地

- tokens 5.16 MB + surface ids 2.67 MB: combo の頻度順 Huffman、同読み内コストの
  差分 varint、表層 id の頻度順並べ替え (T151)
- 変換時のワーキングセット (tmpfs 計測で辞書分 ~6 MB): 各 BitVec の rank 索引・select
  サンプルを本体と別領域に置いているため、1 回の rank/select で 2〜3 ページに触れる。
  索引をブロックにインターリーブすればページ数を減らせる (T150 の続き)
- PRD の embedded 予算 ~2 MB は cgroup MemoryMax 下での完走テストで検証する予定
  だったが、この環境は cgroup v1 (cpu のみ) で memory コントローラが使えず未実施
