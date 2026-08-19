# メモリ使用量

かな漢字変換エンジンとしての実メモリ消費の記録。再計測:

```bash
cargo build --release -p jamo-cli
python3 scripts/bench-memory.py --with-anthy --with-mozc-server
```

## 他エンジン比較 (2026-08-19, Linux x86_64, jamo@3d62bdd)

| エンジン | 辞書データ | peak RSS | 計測条件 |
|---------|-----------|----------|---------|
| jamo + mozc.dict | 83.5 MB (.dict) | **152.4 MB** | 変換1回込み |
| jamo + ipadic.dict | 20.1 MB (.dict) | **42.9 MB** | 変換1回込み |
| Anthy (anthy-unicode 1.0.0.20260213) | 20.3 MB (anthy.dic) | **16.8 MB** | 付属テスト約550件の変換込み |
| mozc_server (本家 fcitx5-mozc) | 22.7 MB (バイナリ埋め込み) | **6.2 MB** | 起動直後・変換なし (下限値) |

- mozc_server はクライアントなしでは変換を実行できないため起動直後の値。
  辞書をバイナリに埋め込み mmap + lazy load するため、変換後も
  バイナリサイズ 22.7 MB + 作業領域が上限の目安。計測環境ではデーモンが
  即終了するため手動起動で2回観測した値 (6.2 / 6.3 MB)。
  `bench-memory.py --with-mozc-server` は観測できない環境では skip する。
- 同じ mozc dictionary_oss ソースを、本家は 22.7 MB のバイナリに収め、
  jamo の .dict は 83.5 MB、ロード後 152 MB。データ量ではなく
  エンジン実装 (フォーマットとロード方式) の差。

## 辞書バイナリ内訳 (フォーマット v2)

`cargo run -p jamo-cli --release -- dict info data/mozc.dict` (2026-08-19):

```
data/mozc.dict (v2, total 83.5 MB)
louds bitvec          1.1 MB    1.3%  8773677 bits
labels                4.4 MB    5.3%  4386839 nodes
terminal             35.1 MB   42.0%  4386839 nodes, sentinel 83.0%
entries              28.7 MB   34.3%  1289076 entries
  surface            17.1 MB   20.5%  uniq 983570 (13.4 MB)
  fixed 9B           11.6 MB   13.9%  left/right/cost/pos_type + len
conn matrix          14.3 MB   17.1%  num_pos 2672
stats: vocab 1289076, readings 745965
```

主な無駄:

1. **ロード方式**: 全エントリを `String`/`Vec` にヒープ展開するため、
   ファイル 83.5 MB が RSS 152 MB に膨張する (約1.8倍)
2. **terminal 35.1 MB**: 83% が非終端ノードのセンチネル (0xFFFFFFFF)。
   終端フラグ BitVec + 終端のみの配列にすれば ~4 MB
3. **恒等表層**: 表層が読みのひらがな/カタカナと一致するエントリが
   31.8% (ひらがな 7.3% + カタカナ 24.5%)、表層バイトの 41% (7.1 MB)。
   読みから生成可能なので格納不要にできる
4. **接続行列 14.3 MB**: num_pos 2672 の素の i16 密行列

恒等表層の計測 (mozc ソーステキストに対して。カタカナ→ひらがな正規化後の
読みと表層を比較):

```python
# data/mozc/dictionary*.txt: 読み\tleft\tright\tcost\t表層
def kata_to_hira(s):
    return ''.join(chr(ord(c)-0x60) if 0x30A1 <= ord(c) <= 0x30F6 else c for c in s)
# surface == kata_to_hira(yomi)        → ひらがな恒等 (7.3%)
# kata_to_hira(surface) == kata_to_hira(yomi) → カタカナ恒等 (24.5%)
```

対処は Iter 9 (doc/tasks.md) を参照。

## mozc_server バイナリ 22.7MB の内訳 (2026-08-19)

比較基準の「本家 22.7MB」の中身を実測した。readelf でコード (.text) は
2.9MB のみで、.rodata 19.2MB のほぼ全体が埋め込み mozc dataset
(magic `\xEF MOZC\r\n` を .rodata 先頭で確認)。dataset 末尾の TOC
(protobuf DataSetMetadata: name/offset/size の46エントリ、合計 18.9MB で
.rodata と整合) をパースした主要チャンク:

| チャンク | サイズ | 対応する jamo セクション |
|---------|-------|------------------------|
| dict (読みtrie + token + 表層) | 14.65 MB | trie + token + 表層プール (v3 見積 15.2MB) |
| conn (接続行列) | 2.22 MB | conn matrix (v3 見積 2.7MB) |
| その他 (絵文字/zero query/単漢字/記号/usage 等) | ~2.0 MB | 対応機能なし |

jamo と同等機能のデータは **dict + conn = 16.87MB**。v3 見積もり 17.92MB は
バイナリ全体 (22.7MB) には勝つが、データ対データでは 1.05MB 及ばない。

## v3 サイズ見積もり (2026-08-19)

`scripts/estimate-v3.py` による mozc 実データでの事前検証。trie ノード数は
ソート済みキーの LCP 法で算出し、v2 の実ノード数 (4386839) と一致することで
手法を検証している。

```
reading trie (1B enc)            2.69 MB
token (4B + 3B non-ident)        7.80 MB
combo table                      0.06 MB
surface pool (encoded)           4.64 MB
conn matrix (varint)             2.73 MB
v3 TOTAL                        17.92 MB  (目標 <= 20 MB、本家 mozc_server 22.7 MB)
```

- 素朴な積み上げ (terminal 配列を残す・表層 UTF-8・行列例外固定長) は
  27.2MB で目標に届かない。エントリ境界 BitVec 化・表層の可変長文字符号化・
  行列例外 varint の3施策で 17.92MB になる
- 表層は trie 化 (4.0MB) と可変長文字符号化 (3.9MB) がほぼ同等 → 実装と
  復元が単純な符号化を採用
- 読みの文字種は 255 で1バイト符号の上限ぎりぎり。追加辞書で増える場合の
  エスケープ設計が必要 (T148)
- RSS 見込み: ファイル 17.9MB (mmap) + rank/select 再構築 + 変換作業領域
  で ~20MB (目標 ≤ 25MB)
