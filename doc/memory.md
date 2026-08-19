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

`scripts/dict-breakdown.py data/mozc.dict` (2026-08-19):

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
