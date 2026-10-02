# jamo 辞書バイナリフォーマット仕様

## バージョン

現行: **v3** (2026-10-01) — ゼロコピー。ファイルレイアウト = メモリレイアウト。
`BinDict` (jamo-core/src/bindict/) はバイト列 (Vec または mmap) を所有し、
検索時に必要な部分だけを借用参照する。ロード時にエントリをヒープに展開しない。

設計の要点:

- 読みは 1 バイト符号化した LOUDS trie (頻度上位 255 文字を 1 バイト、残りはエスケープ)
- エントリ (トークン) は終端ノードの BFS 順に並び、終端 k 番目 → トークン範囲を
  select で引く (終端フラグ BitVec + エントリ境界 BitVec)
- 表層は読みと同一 (ひらがな / カタカナ) なら 2 bit のフラグだけ。それ以外は
  可変長文字符号化した共有プールを参照する
- (left_id, right_id, pos_type) の組は小さな表 (combo) にまとめ、トークンは 14 bit で参照
- 接続行列は行ごとの最頻値をデフォルトにし、例外だけを BitVec + コスト配列で持つ
  (参照は rank 1 回で O(1))
- 全 BitVec は rank 索引 (と必要なら select サンプル) をファイルに含める

## ファイル構造

全ての整数値はリトルエンディアン。各セクションは 8 バイト境界に揃える (間はゼロ詰め)。

```
+-------------------------------+
| Header + TOC          (128 B) |
+-------------------------------+
|  0 reading chars              |  読みの文字表
|  1 louds bitvec               |  trie 構造
|  2 labels                     |  ノードのラベル (1 バイト符号)
|  3 terminal bitvec            |  ノードが終端か
|  4 entry bounds               |  トークン列中の各読みの先頭
|  5 tokens                     |  4 B × トークン数
|  6 combos                     |  5 B × combo 数
|  7 nonident bitvec            |  トークンが表層プールを参照するか
|  8 surface ids                |  3 B × プール参照トークン数
|  9 surface chars              |  表層の文字表
| 10 surface pool               |  符号化した表層の連結
| 11 surface bounds             |  プール中の各表層の先頭
| 12 conn matrix                |  接続行列
| 13 stats                      |  語彙数・読み数
| 14 boundary                   |  文節境界ペナルティ (品詞 ID ごと)
+-------------------------------+
```

## Header + TOC

| Offset | Size | Type | 説明 |
|--------|------|------|------|
| 0 | 4 | bytes | Magic: `JAMO` |
| 4 | 4 | u32 | Version: `3` |
| 8 | 4 | u32 | セクション数: `15` |
| 12 | 4 | u32 | 予約 (0) |
| 16 | 8 × 15 | (u32 offset, u32 len)[] | 各セクションのファイル先頭からのオフセットと長さ |

セクションは TOC の順に並び、重ならない。TOC 外の末尾バイトは無視する。

## BitVec (共通)

rank/select 付きビット列の直列化 (jamo-core/src/bitvec.rs)。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | len: ビット数 |
| 4 | u32 | nblocks: 64bit ブロック数 (= ceil(len / 64)) |
| 4 | u32 | sample: select サンプル間隔 (64、または 0 = サンプルなし) |
| 4 | u32 | 予約 (0) |
| 8 × nblocks | u64[] | ビット本体 (LSB first) |
| 8 × ndir | u64[] | rank 索引。ndir = ceil(nblocks / 4) + 1 |
| 4 × nsel1 | u32[] | select1 サンプル: sample 個ごとの 1 の位置。nsel1 = ceil(ones / sample) |
| 4 × nsel0 | u32[] | select0 サンプル: sample 個ごとの 0 の位置。nsel0 = ceil(zeros / sample) |

rank 索引の 1 エントリは 256 bit (ブロック 4 つ) をカバーする:
bit 0..32 = 大ブロック先頭までの 1 の累積数、bit 32..40 / 40..48 / 48..56 =
2〜4 番目のブロック先頭までの大ブロック内の 1 の数。末尾エントリは番兵
(累積 = 1 の総数) で、`count_ones` を O(1) にする。
rank は索引 1 回 + ブロック 1 回の読み出し、select はサンプルから最大 64 個分を
ブロック単位で走査する。sample = 0 の BitVec では select は使えない (接続行列用)。

## 0: Reading chars / 9: Surface chars

文字表。読みは頻度上位 255 文字、表層は全文字 (上限 73,696)。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | count |
| 4 × count | u32[] | Unicode コードポイント (頻度順) |

### 読みの符号化

文字表の i 番目 (0 始まり) → 1 バイト `i + 1` (1..=255)。表にない文字は
`0` + その文字の UTF-8 バイト列。検索時は入力を 1 文字ずつ符号化して trie を辿り、
文字の符号列を消費し終えた位置でだけ終端を見る。

### 表層の符号化

文字表の i 番目 →
- i < 224: 1 バイト `i`
- 224 ≤ i < 224 + 31 × 256: 2 バイト `224 + (i - 224) / 256`, `(i - 224) % 256`
- それ以降: 3 バイト `255`, 上位, 下位 (i - 224 - 7936 を 256 進で)

## 1: LOUDS bitvec / 2: Labels / 3: Terminal bitvec

LOUDS trie。ビット列はスーパールートの `10` から始まり、各ノードの子の数を
unary (`1...10`) で BFS 順に並べる。ノード番号は 0 = スーパールート、1 = ルート、
以降 BFS 順。ノード n の子ビットは `select0(n - 1) + 1` から始まり、最初の子の
ノード番号は `rank1(その位置) + 1`、子は連番。

- labels: ノード数 (= 1 の数 + 1) バイト。ノード番号順のエッジラベル (符号バイト)。
  同じ親の子はラベル昇順
- terminal: ノード数 bit。終端 (読みの末尾) なら 1。終端ノードの順番 k =
  `rank1(terminal, n)` がトークン列の読み番号になる

## 4: Entry bounds / 5: Tokens / 6: Combos

トークン列は終端ノードの BFS 順 (= 読み番号順) に、各読みのエントリを連続して並べる。

- entry bounds: トークン数 bit。各読みの最初のトークンで 1。読み k のトークン範囲は
  `[select1(k), select1(k + 1))` (末尾はトークン数)
- tokens: 1 トークン 4 バイト

| Size | Type | 説明 |
|------|------|------|
| 2 | u16 | bit 0..14 = combo 番号、bit 14..16 = 表層種別 (0 = 読みそのまま、1 = 読みのカタカナ、2 = 表層プール) |
| 2 | i16 | 単語コスト |

- combos: 1 組 5 バイト (u16 left_id, u16 right_id, u8 pos_type)。最大 16,384 組

## 7: Nonident bitvec / 8: Surface ids / 10: Surface pool / 11: Surface bounds

表層種別 2 のトークンだけが表層プールを参照する。

- nonident: トークン数 bit。種別 2 なら 1。トークン t の表層 id は
  `surface ids[rank1(nonident, t)]`
- surface ids: 3 バイト (u24) × nonident の 1 の数。プール中の表層番号
- surface pool: 符号化した表層の連結 (重複なし、空文字列なし)
- surface bounds: プールのバイト数 bit。各表層の先頭バイトで 1。表層 id の範囲は
  `[select1(id), select1(id + 1))`

## 12: Connection matrix

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | num_pos: 品詞 ID 数 (≤ 8192) |
| 2 × num_pos | i16[] | 行 (right_id) ごとのデフォルトコスト (最頻値) |
| BitVec | | 例外ビット列: num_pos² bit、`right_id * num_pos + left_id` がデフォルトと違えば 1 (select サンプルなし) |
| 2 × 例外数 | i16[] | 例外コスト (ビット順) |

`cost(right, left)`: 例外ビットが立っていれば `costs[rank1(idx)]`、なければ `defaults[right]`。

## 13: Stats

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | vocab_size: トークン数 |
| 4 | u32 | reading_count: 読みの数 |

## 14: Boundary

文節境界ペナルティ (mozc の boundary.def、jamo-core/src/boundary.rs)。長さ 0 (表なし、
全て 0) か `4 × num_pos`。

| Size | Type | 説明 |
|------|------|------|
| 2 | u16 | prefix: 変換キーの先頭に来るノード (left_id = この ID) の単語コストに足す |
| 2 | u16 | suffix: 変換キーの末尾に来るノード (right_id = この ID) の単語コストに足す |

## 検証

- `BinDict::open` / `from_vec`: ヘッダ、TOC、各セクションのサイズと個数の整合
  (ノード数 = labels 長 = terminal 長、トークン数 = entry bounds 長 = nonident 長、
  終端数 = entry bounds の 1 の数、プール参照数 = surface ids 数、行列サイズ) を検証する。
  ファイル全体には触れない (mmap の遅延ロードを保つ)
- `BinDict::verify` (`jamo dict info` が実行): 全 BitVec の rank 索引・select サンプル、
  トークンの combo 番号、表層 id の範囲を検証する
- 検証を通った内容が壊れていても (例: ビット本体の改変) 変換結果が壊れるだけで
  panic はしない (全ての参照は境界検査つき)

## 実測 (mozc dictionary_oss、2026-10-01)

```
data/mozc.dict (v3, total 19.3 MB)
reading chars        0.00 MB    0.0%
louds bitvec         0.80 MB    4.2%
labels               1.84 MB    9.5%
terminal bitvec      0.40 MB    2.1%
entry bounds         0.28 MB    1.5%
tokens               5.16 MB   26.8%
combos               0.06 MB    0.3%
nonident bitvec      0.28 MB    1.5%
surface ids          2.67 MB   13.9%
surface chars        0.02 MB    0.1%
surface pool         3.99 MB   20.7%
surface bounds       0.87 MB    4.5%
conn matrix          2.89 MB   15.0%
boundary             0.01 MB    0.1%
trie nodes 1837020, tokens 1289076, id combos 11370, pool surfaces 698652,
matrix 2672x2672 (exceptions 884398)
```

v2 (83.5 MB) との比較: terminal 35.1 MB → 0.7 MB (フラグ + 境界 BitVec)、
entries 28.7 MB → 12.8 MB (combo 表 + 恒等フラグ + 符号化プール)、
labels + louds 5.5 MB → 2.6 MB (1 バイト符号)、行列 14.3 MB → 2.9 MB。
IPAdic は 19.2 MB → 8.4 MB。

## 変更履歴

| Version | 変更内容 |
|---------|---------|
| 1 | 初版。LOUDS trie + 接続コスト行列。 |
| 2 | Entry に pos_type (1 byte) を追加。 |
| 3 | ゼロコピー化。読み 1 バイト符号、終端/境界 BitVec、combo 表、恒等表層フラグ、表層プール符号化、行列の例外圧縮、rank/select 索引の内蔵。v2 は読めない。同日に boundary セクション (14) を追加。 |
