# jamo 辞書バイナリフォーマット仕様

## バージョン

現行: **v2** (2026-02-24)

## ファイル構造

全ての整数値はリトルエンディアン。

```
+-------------------+
| Header            |
+-------------------+
| LOUDS BitVec      |
+-------------------+
| Labels            |
+-------------------+
| Terminal          |
+-------------------+
| Entries           |
+-------------------+
| Connection Matrix |
+-------------------+
| Stats             |
+-------------------+
```

## Header

| Offset | Size | Type | 説明 |
|--------|------|------|------|
| 0 | 4 | bytes | Magic: `JAMO` (0x4A 0x41 0x4D 0x4F) |
| 4 | 4 | u32 | Version: `2` |

## LOUDS BitVec

LOUDS trie の構造を表現するビットベクトル。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | bit_len: ビット数 |
| 4 | u32 | block_count: 64bit ブロック数 |
| block_count * 8 | u64[] | ビットデータ (各ブロック 64bit, LE) |

rank/select テーブルはロード時に再構築。

## Labels

各 LOUDS ノードのエッジラベル (バイト値)。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | label_count: ノード数 |
| label_count | u8[] | ラベル配列 |

## Terminal

各ノードがキーの終端かどうか、および対応するエントリの範囲。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | terminal_count: ノード数 |
| terminal_count * 8 | (u32, u32)[] | (start, end) ペア |

- `start == 0xFFFFFFFF`: 終端ではない (None)
- それ以外: Entries プール内のインデックス範囲 `[start, end)`

## Entries

辞書エントリのプール。Terminal の (start, end) で参照される。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | entry_count: エントリ数 |

各エントリ (可変長):

| Size | Type | 説明 |
|------|------|------|
| 2 | u16 | surface_len: 表層形のバイト長 |
| surface_len | bytes | surface: 表層形 (UTF-8) |
| 2 | u16 | left_id: 左文脈ID (品詞ID) |
| 2 | u16 | right_id: 右文脈ID (品詞ID) |
| 2 | i16 | cost: 生起コスト |
| 1 | u8 | pos_type: 品詞タイプ (0=自立語, 1=付属語) |

## Connection Matrix

品詞間の接続コスト行列。`matrix[right_id * num_pos + left_id]` でアクセス。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | num_pos: 品詞ID数 |
| num_pos * num_pos * 2 | i16[] | 接続コスト行列 (LE) |

## Stats

辞書の統計情報。

| Size | Type | 説明 |
|------|------|------|
| 4 | u32 | vocab_size: 表層形のユニーク数 |
| 4 | u32 | reading_count: 読みのユニーク数 |

## 変更履歴

| Version | 変更内容 |
|---------|---------|
| 1 | 初版。LOUDS trie + 接続コスト行列。 |
| 2 | Entry に pos_type (1 byte) を追加。 |
