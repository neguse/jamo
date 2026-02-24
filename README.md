# Jamo - かな漢字変換エンジン & API

省メモリで組み込み可能なかな漢字変換エンジン (Rust) と、Cloudflare Workers 上で動作する変換 API。

## 特徴

- **Rust 製変換エンジン** — LOUDS trie + Viterbi デコーダによるかな漢字変換
- **複数インターフェース** — CLI / HTTP API / WebSocket / WASM / C API
- **Cloudflare Workers 対応** — Durable Objects でユーザーごとに独立した辞書・学習データ
- **辞書マージ** — IPAdic ベース + NEologd 等の追加辞書を統合可能
- **省メモリ** — WASM バイナリ 144KB、辞書バイナリ 19.2MB

## クイックスタート

### 前提条件

- Rust (edition 2024)
- IPAdic 辞書データ (`data/ipadic/` に配置)

### ビルド

```bash
cargo build --release
```

### CLI で変換

```bash
# 辞書バイナリを構築 (初回のみ)
cargo run -- dict build -o data/ipadic.dict

# かな漢字変換
cargo run -- convert きょうはいいてんきですね
# → 今日はいい天気ですね
```

### HTTP サーバー

```bash
cargo run -- serve --port 8080

# 変換
curl 'http://localhost:8080/convert?q=きょうはいいてんきですね'

# 予測変換
curl 'http://localhost:8080/predict?q=きょう'
```

### 辞書マージ (NEologd 等)

```bash
# ベース辞書 + 追加 CSV をマージ
cargo run -- dict merge --base data/ipadic --extra path/to/neologd/ -o data/merged.dict

# 複数の追加ソースも可能
cargo run -- dict merge --base data/ipadic --extra extra1.csv --extra extra2/ -o merged.dict
```

追加 CSV は IPAdic と同じ形式 (13フィールド、カンマ区切り):

```
表層形,左文脈ID,右文脈ID,コスト,品詞,品詞細分類1,品詞細分類2,品詞細分類3,活用型,活用形,原形,読み,発音
```

## HTTP API

| エンドポイント | メソッド | 説明 |
|---------------|---------|------|
| `/health` | GET | ヘルスチェック |
| `/convert?q=...` | GET | かな漢字変換 |
| `/resize?q=...&boundaries=...&clause=...&delta=...` | GET | 文節伸縮 |
| `/predict?q=...` | GET | 予測変換 |
| `/commit` | POST | 変換確定 (学習) |
| `/dict/add` | POST | ユーザー辞書に単語追加 |
| `/dict/remove` | POST | ユーザー辞書から単語削除 |
| `/dict/list` | GET | ユーザー辞書一覧 |
| `/dict/import` | POST | ユーザー辞書一括インポート |
| `/ws` | GET | WebSocket 接続 |

詳細は [doc/api.yaml](doc/api.yaml) (OpenAPI 3.0.3) を参照。

## WASM ビルド

```bash
cd jamo-core
wasm-pack build --target web --features wasm
```

## C API

```bash
# 共有ライブラリをビルド
cargo build --release -p jamo-core

# ヘッダ生成
cbindgen --config jamo-core/cbindgen.toml -o include/jamo.h jamo-core

# 使用例
# jamo_engine_t* engine = jamo_engine_open("data/ipadic.dict");
# char* result = jamo_convert(engine, "きょう", 5);
# jamo_string_free(result);
# jamo_engine_free(engine);
```

ヘッダ: [jamo-core/include/jamo.h](jamo-core/include/jamo.h)

## Cloudflare Workers デプロイ

```bash
cd jamo-worker
npm install
wrangler deploy
```

R2 バケットに辞書バイナリ (`ipadic.dict`) をアップロード:

```bash
wrangler r2 object put DICT_BUCKET/ipadic.dict --file data/ipadic.dict
```

## Windows IME

Windows 用の TSF (Text Services Framework) IME として動作する DLL。ローマ字入力→ひらがな→漢字変換を提供。

### ビルド

```bash
cargo build --release -p jamo-ime
```

成果物: `target/release/jamo_ime.dll`

### インストール

1. 辞書バイナリを構築 (未作成の場合):

```bash
cargo run -- dict build -o data/ipadic.dict
```

2. DLL と辞書を配置:

```powershell
mkdir "$env:LOCALAPPDATA\Jamo"
copy target\release\jamo_ime.dll "$env:LOCALAPPDATA\Jamo\"
copy data\ipadic.dict "$env:LOCALAPPDATA\Jamo\"
```

3. DLL を登録 (管理者権限不要):

```powershell
regsvr32 "$env:LOCALAPPDATA\Jamo\jamo_ime.dll"
```

4. Windows の設定 → 時刻と言語 → 言語 → 日本語 → キーボード で「Jamo」を選択。

### 辞書の配置場所

以下の順序で辞書ファイル `ipadic.dict` を検索する:

1. レジストリ `HKCU\Software\Jamo\DictPath` に指定されたパス
2. DLL と同じディレクトリ
3. `%APPDATA%\Jamo\ipadic.dict`

### アンインストール

```powershell
regsvr32 /u "$env:LOCALAPPDATA\Jamo\jamo_ime.dll"
```

### 操作方法

| キー | 動作 |
|------|------|
| 半角/全角 | IME オン/オフ切替 |
| 英数字キー | ローマ字入力 → ひらがな変換 |
| Space | かな漢字変換 / 次の候補 |
| Enter | 確定 |
| Escape | 変換取消 |
| ←/→ | 文節移動 |
| Shift+←/→ | 文節伸縮 |
| Backspace | 1文字削除 |

## プロジェクト構成

```
jamo/
├── jamo-core/      変換エンジン (Rust lib)
├── jamo-cli/       CLI / HTTP サーバー / 辞書ビルド (Rust bin)
├── jamo-ime/       Windows IME (TSF DLL, cdylib)
├── jamo-worker/    Cloudflare Worker / Durable Object (TypeScript)
├── data/           辞書データ
│   ├── ipadic/     IPAdic ソース CSV
│   └── ipadic.dict コンパイル済み辞書バイナリ
└── doc/            ドキュメント
    ├── api.yaml    API 仕様 (OpenAPI 3.0.3)
    └── dict-format.md  辞書バイナリフォーマット仕様
```

## パフォーマンス

| 項目 | 値 |
|------|------|
| 変換 (短文) | 10 us |
| 変換 (中文) | 87 us |
| 変換 (長文) | 99 us |
| 辞書ロード | 64 ms |
| 予測変換 | 1-2 us |
| WASM バイナリ | 144 KB |
| 辞書バイナリ | 19.2 MB |

## ライセンス

MIT
