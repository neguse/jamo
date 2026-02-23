# Jamo - かな漢字変換エンジン & API

## 概要

Jamo は、省メモリで組み込み可能なかな漢字変換エンジンと、それを基盤とするセルフホスト可能な変換APIである。Cloudflare Durable Objects 上でユーザーごとに隔離された変換環境を提供し、ユーザー辞書・学習データの永続化をサーバーレスで実現する。

Yahoo かな漢字変換API（サービス終了済み）、Google CGI API（非公式・不安定）の OSS 代替として、誰でもデプロイ・カスタマイズ可能なかな漢字変換基盤を目指す。

## 背景と課題

- Yahoo かな漢字変換APIはサービス終了済み
- Google CGI APIは非公式であり、いつ廃止されてもおかしくない
- mozc はサーバーモードが存在するが、APIとして設計されておらずセットアップが困難
- アプリ開発者・研究者・IME開発者が自由に使える変換APIの選択肢がない
- 既存の商用IME（ATOK等）は辞書カスタマイズの自由度が低く、ドメイン特化が困難

## ゴール

1. `wrangler deploy` 一発でデプロイ可能なかな漢字変換API
2. ユーザーごとに隔離された辞書・学習データ（Durable Objects）
3. ドメイン特化辞書の差し替えによる特定分野での高精度変換
4. 組み込み用途にも使えるライブラリ（C API公開）としての提供
5. オープンな辞書エコシステムの構築

## ノンゴール

- 汎用IMEとしてATOKやGoogle日本語入力の全機能を代替すること
- GUIを持つIMEフロントエンドの開発
- リアルタイム音声入力・手書き入力への対応

## 互換性ポリシー

**1.0 リリースまで後方互換性は一切保証しない。** API、辞書フォーマット、設定、内部構造は予告なく破壊的に変更する。作っては壊して学び、最も綺麗な設計にたどり着くことを優先する。1.0 以降はセマンティックバージョニングに従う。

## ターゲットユーザー

| ユーザー | ユースケース |
|---------|------------|
| アプリ開発者 | 自前アプリに変換機能を組み込みたい |
| 研究者 | 変換精度の実験、形態素解析パイプラインへの組み込み |
| IME開発者 | 変換エンジンを自作せずUIだけ作りたい |
| 企業 | 社内用語に対応した変換APIが欲しい |
| 組み込み開発者 | Kindle, Raspberry Pi Zero 等の低リソース環境でIMEを動かしたい |

## 技術スタック

| レイヤー | 言語 | 役割 |
|---------|------|------|
| 変換エンジン (jamo-core) | Rust | コアロジック。`#[no_std]` 対応、cbindgenでC API公開、wasm-packでWASM出力 |
| CLI / 辞書ビルド / HTTPサーバー (jamo-cli) | Rust | jamo-coreをライブラリとして直接利用。単一バイナリで全ツールを提供 |
| CF Worker / Durable Object (jamo-worker) | TypeScript | CF上のグルーコード。wasm-packの出力を利用 |

### 言語選定理由

- **Rust**: エンジンからCLIまで1言語で完結し、FFIの境界がない。UTF-8が言語レベルで安全に扱える。`#[no_std]` + アリーナアロケータで組み込み対応可能。`wasm-pack` によるCF Workers連携が成熟。`cbindgen` でC APIヘッダを自動生成し、C/C++からの利用も可能。
- **TypeScript**: Cloudflare Workers/Durable Objectsのネイティブ言語。WASMラッパーとHTTPルーティングのみを担当する薄い層。

## システムアーキテクチャ

### 全体構成

```
Client (HTTP/WebSocket)
  │
  ▼
┌─────────────────────────┐
│  Cloudflare Worker       │  ルーティング・認証 (TypeScript)
│  - JWT / APIキー検証      │
│  - user_id 抽出          │
│  - Durable Object へ転送  │
└──────────┬──────────────┘
           │
           ▼
┌─────────────────────────┐
│  Durable Object          │  ユーザーごとに1インスタンス (TypeScript)
│  (id = user_id)          │
│                          │
│  ┌───────────────┐       │
│  │ WASM Engine   │       │  Rust → wasm-pack
│  │ (jamo.wasm)   │       │
│  └───────┬───────┘       │
│          │               │
│  ┌───────▼───────┐       │
│  │ SQLite Storage │      │  ユーザー辞書・学習データ
│  └───────────────┘       │
└─────────────────────────┘
```

```
セルフホスト時:

$ jamo serve                    # HTTPサーバー起動
$ jamo dict build corpus.txt    # 辞書ビルド
$ jamo dict merge a.dict b.dict # 辞書マージ
$ jamo dict info system.dict    # 辞書統計
$ jamo convert きょうはいいてんき  # CLIで変換テスト

全て単一バイナリ (Rust) で提供。
```

### プロジェクト構成

```
jamo/
├── jamo-core/          # 変換エンジン (Rust, #[no_std] 対応)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── trie.rs         # LOUDS trie
│   │   ├── lattice.rs      # ラティス構築
│   │   ├── viterbi.rs      # Viterbiデコーダ
│   │   ├── lm.rs           # 言語モデル (bigram)
│   │   ├── dict.rs         # 辞書読み込み
│   │   ├── user_dict.rs    # ユーザー辞書
│   │   └── ffi.rs          # C API (cbindgen)
│   └── Cargo.toml
│
├── jamo-cli/           # CLI・サーバー・辞書ビルド (Rust)
│   ├── src/
│   │   ├── main.rs
│   │   ├── cmd_serve.rs    # HTTPサーバー (axum)
│   │   ├── cmd_convert.rs  # CLI変換
│   │   └── cmd_dict.rs     # 辞書ビルド・マージ
│   └── Cargo.toml
│
├── jamo-worker/        # Cloudflare Worker/DO (TypeScript)
│   ├── src/
│   │   ├── index.ts        # Worker ルーター
│   │   └── do.ts           # Durable Object
│   ├── wrangler.toml
│   └── package.json
│
├── dicts/              # 辞書ソース・ビルド済みバイナリ
├── Cargo.toml          # ワークスペース
└── PRD.md
```

### 変換エンジン (jamo-core)

かな漢字変換のコアロジック。feature flagで組み込み向けとデスクトップ向けを切り替え可能。

```
変換パイプライン:
  入力(かな列) → 辞書引き(共通接頭辞検索) → ラティス構築 → Viterbiデコード → N-best候補
```

#### コンポーネント

| コンポーネント | 実装 | 役割 |
|--------------|------|------|
| 辞書 | LOUDS trie | 省メモリなかな→語彙ID検索 |
| 品詞接続コスト | i16 行列 (256x256) | 文節区切り精度の担保 |
| 言語モデル | 圧縮bigram (量子化8bit) | 自然な変換候補の順序付け |
| デコーダ | Viterbi on lattice | 最適な変換パスの探索 |
| ユーザー辞書 | ハッシュテーブル | ユーザー定義語彙 |
| 学習テーブル | LRUハッシュテーブル | 変換履歴による適応 |

#### 設計制約

- `#[no_std]` 対応（`alloc` crateのみ使用可、feature flagで切り替え）
- 組み込みプロファイルでは動的メモリ確保を回避可能（アリーナアロケータ）
- 浮動小数点演算なし（全て i16/i32 固定小数点）
- `cbindgen` でC APIヘッダ自動生成 → C/C++からリンク可能
- `wasm-pack` でWASMモジュール出力 → CF Workers/ブラウザで動作

#### ビルドターゲット

| ターゲット | ビルド方法 | 用途 |
|-----------|-----------|------|
| ネイティブ (x86_64/aarch64) | `cargo build --release` | CLI、サーバー、組み込み |
| WASM | `wasm-pack build` | CF Workers、ブラウザ |
| C静的ライブラリ | `cargo build --release` + cbindgen | C/C++アプリへの組み込み |
| 組み込み (ARM) | `cargo build --target thumbv7em-none-eabi` | Kindle等 |

#### メモリ予算

| 構成 | 対象 | 合計メモリ |
|------|------|-----------|
| embedded | Kindle, Raspi Zero | ~2 MB |
| desktop | Windows Laptop | ~50 MB |
| wasm | Cloudflare DO | ~6 MB |

### Durable Object 層

- ユーザーごとに独立したDOインスタンスを生成
- DO内蔵SQLiteにユーザー辞書・学習データを永続化
- idle時は自動hibernate（メモリ解放・課金停止）
- 再アクセス時にSQLiteから状態復元

### 辞書データ

システム辞書はWASMバイナリに埋め込み（~5MB）。ドメイン辞書はR2に配置し、オンデマンドでロード。

辞書ソース（OSS互換ライセンス）:
- IPAdic: 基本語彙・品詞体系 (BSD系)
- NEologd: 新語・固有名詞 (Apache 2.0)
- JMdict: 補完 (CC-BY-SA)

## API仕様

### 認証

```
Authorization: Bearer <JWT or API key>
```

JWTからuser_idを抽出し、対応するDurable Objectにルーティングする。

### エンドポイント

#### `GET /convert` - かな漢字変換

```
GET /convert?q=きょうはいいてんきですね&n=3&domain=general

Response:
{
  "result": [
    {
      "yomi": "きょうは",
      "candidates": ["今日は", "京は", "教は"],
      "pos": "名詞+助詞"
    },
    {
      "yomi": "いい",
      "candidates": ["いい", "良い", "好い"]
    },
    {
      "yomi": "てんき",
      "candidates": ["天気", "天機", "転機"]
    },
    {
      "yomi": "ですね",
      "candidates": ["ですね"]
    }
  ]
}
```

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| q | string | (必須) | 変換対象の読み (UTF-8) |
| n | int | 5 | 各文節の候補数 |
| domain | string | "general" | 辞書ドメイン |

#### `GET /candidates` - 単語候補

```
GET /candidates?q=かんじ&n=10

Response:
{
  "candidates": [
    {"surface": "漢字", "pos": "名詞"},
    {"surface": "感じ", "pos": "名詞"},
    {"surface": "幹事", "pos": "名詞"},
    ...
  ]
}
```

#### `GET /predict` - 予測変換

```
GET /predict?q=きょうは&n=5

Response:
{
  "predictions": ["今日はいい天気", "今日はお疲れ様", "今日はありがとう"]
}
```

#### `POST /commit` - 候補確定（学習）

```
POST /commit
{
  "yomi": "きょうは",
  "selected": 0,
  "context": ["昨日", "は", "雨", "でした"]
}

Response:
{"ok": true}
```

ユーザーが候補を確定したことをエンジンに通知する。学習テーブルが更新され、次回以降の変換精度が向上する。

#### `POST /dict/add` - ユーザー辞書追加

```
POST /dict/add
{
  "yomi": "くらうど",
  "surface": "Claude",
  "pos": "名詞-固有名詞"
}

Response:
{"ok": true}
```

#### `DELETE /dict/remove` - ユーザー辞書削除

```
DELETE /dict/remove
{
  "surface": "Claude"
}

Response:
{"ok": true}
```

#### `GET /dict/list` - ユーザー辞書一覧

```
GET /dict/list

Response:
{
  "entries": [
    {"yomi": "くらうど", "surface": "Claude", "pos": "名詞-固有名詞"}
  ]
}
```

#### `POST /dict/import` - ユーザー辞書一括インポート

```
POST /dict/import
Content-Type: text/csv

くらうど,Claude,名詞-固有名詞
あんそろぴっく,Anthropic,名詞-固有名詞

Response:
{"imported": 2}
```

### WebSocket

リアルタイム変換用。HTTP APIと同じ操作をWebSocket上で行える。

```
ws://jamo.example.com/ws

→ {"type": "convert", "q": "きょうは", "n": 5}
← {"type": "result", "result": [...]}

→ {"type": "commit", "selected": 0}
← {"type": "ok"}
```

IMEフロントエンドとの連携では、キーストロークごとに変換リクエストを送信し、低レイテンシで候補を返す。DOとの接続を維持することでhibernationを抑制し、セッション中の応答速度を安定させる。

## C API (cbindgen自動生成)

組み込み・デスクトップ用途向け。Rustの `#[no_mangle] pub extern "C" fn` から cbindgen でヘッダを自動生成する。

```c
/* jamo.h - cbindgenにより自動生成 */
typedef struct JamoCtx JamoCtx;

/* ライフサイクル */
JamoCtx *jamo_open(const char *dict_path);
void     jamo_close(JamoCtx *ctx);

/* 変換 */
int32_t jamo_convert(JamoCtx *ctx,
                     const char *yomi,
                     char *out, size_t out_len,
                     int32_t n_best);

/* 文節操作 */
int32_t jamo_resize_clause(JamoCtx *ctx, int32_t clause_idx, int32_t delta);

/* 候補確定（学習） */
void jamo_commit(JamoCtx *ctx, int32_t candidate_index);

/* ユーザー辞書 */
int32_t jamo_dict_add(JamoCtx *ctx,
                      const char *yomi, const char *surface,
                      const char *pos, int32_t cost);
int32_t jamo_dict_remove(JamoCtx *ctx, const char *surface);
```

## 辞書ビルドパイプライン

`jamo dict build` コマンド（jamo-cli内）でコーパスから辞書バイナリを生成する。

```
テキストコーパス / IPAdic CSV
  │
  ▼
┌──────────────────┐
│  語彙抽出         │  表層形、読み、品詞、頻度
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  LM学習          │  bigram/trigram コスト計算・量子化
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  辞書パック       │  LOUDS trie構築、バイナリ出力
└────────┬─────────┘
         │
         ▼
    system.dict (単一バイナリファイル)
```

```bash
# 使い方
jamo dict build --corpus corpus.txt --output system.dict
jamo dict merge --base system.dict --extra neologd.csv --output merged.dict
jamo dict info system.dict
```

## ドメイン辞書

ドメインごとに辞書+LMのペアを用意し、APIリクエスト時に `domain` パラメータで切り替える。

```
dicts/
  general.dict      # 汎用（デフォルト）
  medical.dict      # 医療
  legal.dict        # 法律
  it.dict           # IT
```

コミュニティが辞書を作成・公開できるエコシステムを構築する。辞書フォーマットとビルドツールを公開し、誰でもドメイン辞書を作れるようにする。

## ロードマップ

### Phase 1: コアエンジン

Rustライブラリ (jamo-core) として変換エンジンの最小実装を完成させる。

- [ ] Cargo ワークスペース構成 (jamo-core, jamo-cli)
- [ ] LOUDS trie の実装 (構築・共通接頭辞検索)
- [ ] 品詞接続コスト行列
- [ ] bigram 言語モデル（量子化8bit）
- [ ] Viterbi デコーダ（ラティス構築・最適パス探索）
- [ ] 連文節変換（文節区切り・N-best）
- [ ] 文節伸縮
- [ ] C API (cbindgen)
- [ ] 辞書ビルド (`jamo dict build`)
- [ ] IPAdic ベースのシステム辞書生成
- [ ] CLIテストコマンド (`jamo convert`)
- [ ] テストスイート

### Phase 2: サーバー & Cloudflare デプロイ

HTTPサーバーとWASM化、CF Workers/DO上での動作。

- [ ] HTTPサーバー (`jamo serve`, axum)
- [ ] HTTP API (`/convert`, `/candidates`)
- [ ] wasm-pack ビルド
- [ ] Cloudflare Worker (ルーター・認証)
- [ ] Durable Object (ユーザーインスタンス)
- [ ] DO SQLite によるユーザー辞書永続化
- [ ] WebSocket API
- [ ] wrangler deploy でのデプロイ確認

### Phase 3: 学習と辞書エコシステム

ユーザー適応と辞書の拡充。

- [ ] 候補確定による学習 (`/commit`)
- [ ] ユーザー辞書 API (`/dict/*`)
- [ ] ドメイン辞書切り替え (`?domain=`)
- [ ] NEologd 統合
- [ ] 予測変換 (`/predict`)

### Phase 4: 精度強化 (デスクトップ向け)

Windows Laptop等の十分なリソースがある環境向け。

- [ ] trigram 言語モデル
- [ ] INT8 NN リランキング (AVX2/NEON)
- [ ] 長文脈変換
- [ ] マルチスレッドパイプライン (N-gram即時表示 + NN非同期リランク)
