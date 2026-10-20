# Jamo - ポータブルなかな漢字変換エンジン

ポータブルなかな漢字変換エンジン (Rust)。ひとつのコアが組み込み機器からサーバーまで同じように動く。

**開発状態: pre-1.0。** API・辞書フォーマット・内部構造は予告なく変わる。

## 特徴

- **ポータブル** — 単一の Rust コアを C API (組み込み) / CLI / HTTP API / WASM / Windows IME (TSF) から利用可能
- **小さいエンジン** — ゼロコピー辞書 (mmap) + LOUDS trie + Viterbi デコーダ。mozc 辞書で .dict 19.3MB、変換時 peak RSS 9.5MB (4KB ページ環境)、WASM バイナリ 144KB。実測は [doc/memory.md](doc/memory.md)
- **辞書ソース** — mozc dictionary_oss (推奨、Top-1 96.4%、本家 mozc_server と同点) / IPAdic + NEologd 等のマージ
- **本家比較** — 同一コーパスで mozc_server 比 レイテンシ 1/20、変換時 RSS 1/3 ([doc/bench.md](doc/bench.md))
- **Cloudflare Workers 対応** — Durable Objects でユーザーごとに独立した辞書・学習データ

## クイックスタート

### 前提条件

- Rust (edition 2024)
- IPAdic 辞書データ: `./scripts/fetch-ipadic.sh` で `data/ipadic/` に取得

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

| エンドポイント | メソッド | 説明 | ローカル (`jamo serve`) | Cloudflare Worker |
|---------------|---------|------|:---:|:---:|
| `/health` | GET | ヘルスチェック | ✓ | ✓ |
| `/convert?q=...` | GET | かな漢字変換 | ✓ | ✓ |
| `/resize?q=...&boundaries=...&clause=...&delta=...` | GET | 文節伸縮 | ✓ | ✓ |
| `/predict?q=...` | GET | 予測変換 | ✓ | ✓ |
| `/commit` | POST | 変換確定 (学習) | — | ✓ |
| `/dict/add` | POST | ユーザー辞書に単語追加 | — | ✓ |
| `/dict/remove` | POST | ユーザー辞書から単語削除 | — | ✓ |
| `/dict/list` | GET | ユーザー辞書一覧 | — | ✓ |
| `/dict/import` | POST | ユーザー辞書一括インポート | — | ✓ |
| `/ws` | GET | WebSocket 接続 | — | ✓ |

学習・ユーザー辞書・WebSocket は Worker (Durable Object がユーザーごとに状態を持つ) だけが提供する。

詳細は [doc/api.yaml](doc/api.yaml) (OpenAPI 3.0.3) を参照。

Cloudflare Worker 版は `Authorization: Bearer <JWT>` が必要。JWT は HS256 で署名し、`sub` をユーザーIDとして扱う。開発時だけ `ALLOW_DEV_USER=true` を設定すると `?user=<id>` も使える。

## WASM ビルド

```bash
cd jamo-core
wasm-pack build --target web --features wasm
```

## 辞書 (mozc dictionary_oss 推奨)

```bash
# mozc 辞書データを取得してバイナリ構築 (Top-1 89.3% / Top-5 98.2%)
scripts/fetch-mozc.sh
cargo run -p jamo-cli --release -- dict build --dict data/mozc -o data/mozc.dict
```

辞書ディレクトリに `id.def` があれば mozc 形式、なければ IPAdic CSV として自動判別する。
mozc 辞書データのライセンスは IPAdic license + BSD-3-Clause (mozc) + Okinawa dictionary
license (詳細は取得される `data/mozc/README.txt`)。辞書バイナリを配布する場合は
これらの帰属表示を同梱すること。

## C API

```bash
# 共有ライブラリをビルド
cargo build --release -p jamo-core

# ヘッダ生成
cbindgen --config jamo-core/cbindgen.toml -o include/jamo.h jamo-core

# 使用例 (v2: セッション型。返却文字列は次の呼び出しまで有効、解放不要)
# jamo_engine_t* engine = jamo_engine_open("data/ipadic.dict");
# jamo_session_t* s = jamo_session_new(engine);
# jamo_session_input(s, "kyouha");
# jamo_session_convert(s, 5);
# const char* c0 = jamo_session_candidate(s, 0, 0);
# const char* text = jamo_session_commit(s);
# jamo_session_free(s);
# jamo_engine_free(engine);
```

ヘッダ: [jamo-core/include/jamo.h](jamo-core/include/jamo.h)

## Cloudflare Workers デプロイ

```bash
cd jamo-worker
npm install
wrangler secret put JWT_SECRET
npm run deploy
```

R2 バケットに辞書バイナリ (`ipadic.dict`) をアップロード:

```bash
wrangler r2 object put jamo-dicts/ipadic.dict --file ../data/ipadic.dict
```

`npm run deploy` は事前に `npm run build-wasm` を実行し、`jamo-core/pkg` を生成してからデプロイする。

## Windows IME

Windows 用の TSF (Text Services Framework) IME として動作する DLL。ローマ字入力→ひらがな→漢字変換を提供。
DLL は辞書を持たず、Named Pipe (`\\.\pipe\jamo-ime`) 経由で IME IPC サーバー (`jamo-cli serve --ime`) に変換を任せる。
サーバーが起動していないと変換できない (サーバーは自動起動しない)。

### ビルド

```bash
cargo build --release -p jamo-ime -p jamo-cli
```

成果物: `target/release/jamo_ime.dll` (IME) と `target/release/jamo-cli.exe` (IPC サーバー)

### インストール

1. 辞書バイナリを構築 (未作成の場合):

```bash
cargo run -p jamo-cli --release -- dict build -o data/ipadic.dict
```

2. 管理者権限の PowerShell を開き、DLL を配置して登録:

```powershell
mkdir "C:\Program Files\Jamo"
copy target\release\jamo_ime.dll "C:\Program Files\Jamo\"
regsvr32 "C:\Program Files\Jamo\jamo_ime.dll"
```

3. IME IPC サーバーを起動する。IME を使う間は起動したままにする:

```powershell
target\release\jamo-cli.exe serve --ime --dict data\ipadic.dict
```

4. Windows の設定 → 時刻と言語 → 言語 → 日本語 → キーボード で「Jamo」を選択。

`--dict` を省くと、カレントディレクトリからの相対パス `data/mozc.dict` → `data/ipadic.dict` の順に辞書を探す。
DLL は未接続なら要求のたびに接続を試みるので、サーバーは IME を選んだ後に起動してもよい。

### アンインストール

```powershell
regsvr32 /u "C:\Program Files\Jamo\jamo_ime.dll"
```

### 操作方法

| キー | 動作 |
|------|------|
| 半角/全角 / Alt+` | IME オン/オフ切替 |
| 英数字キー | ローマ字入力 → ひらがな変換 |
| Space | かな漢字変換 / 次の候補 |
| Enter | 確定 |
| Escape | 変換取消 |
| ←/→ | 文節移動 |
| Shift+←/→ | 文節伸縮 |
| Backspace | 1文字削除 |

## Linux IME (fcitx5)

fcitx5 の入力メソッドアドオン。Wayland (text-input-v3)・XIM・Qt/GTK モジュールの各経路は fcitx5 が受け持つ。
必要なもの: fcitx5 (5.1.12 以降、開発ヘッダ込み)、CMake、C++20 コンパイラ。

### ビルドとテスト

```bash
cmake -S jamo-fcitx5 -B jamo-fcitx5/build -DCMAKE_BUILD_TYPE=Release
cmake --build jamo-fcitx5/build
ctest --test-dir jamo-fcitx5/build   # data/mozc.dict がなければ skip
```

### インストール (`~/.local`)

```bash
cmake --install jamo-fcitx5/build --prefix ~/.local
mkdir -p ~/.local/share/jamo && cp data/mozc.dict ~/.local/share/jamo/
```

fcitx5 はアドオンの共有ライブラリを `FCITX_ADDON_DIRS` (既定 `/usr/lib/fcitx5`) からしか探さないので、
fcitx5 の起動環境に追加する。Omarchy では systemd の drop-in を置く:

```ini
# ~/.config/systemd/user/omarchy-fcitx5.service.d/jamo.conf
[Service]
Environment=FCITX_ADDON_DIRS=%h/.local/lib/fcitx5:/usr/lib/fcitx5
```

```bash
systemctl --user daemon-reload && systemctl --user restart omarchy-fcitx5.service
```

fcitx5-configtool などで入力メソッド「Jamo」を追加する。IME のオン/オフは fcitx5 の入力メソッド切替
(既定 Ctrl+Space) で行う。アンインストールは上記ファイルを消して fcitx5 を再起動する。

辞書は `$JAMO_DICT` → `~/.local/share/jamo/mozc.dict` → `~/.local/share/jamo/ipadic.dict` の順に探す。
学習データは `~/.local/share/jamo/user.dat` に保存する。

### 操作方法

| キー | 入力中 | 変換中 |
|------|--------|--------|
| Space / 変換 | かな漢字変換 | 候補一覧を出して次の候補 |
| ↑/↓ | — | 候補一覧を出して前/次の候補 |
| 1-9 | — | 一覧の候補を選択 |
| ←/→ | カーソル移動 | 文節移動 |
| Shift+←/→ | — | 文節伸縮 |
| Ctrl+↓ | — | フォーカス文節までを確定 |
| Enter | ひらがなのまま確定 | 確定 |
| Escape | 入力を破棄 | 変換取消 |
| F6 / F7 / F9 / F10 | ひらがな / カタカナ / 全角英字 / 半角英字 | 同左 |
| Shift+英字 | 英字モード (確定まで英字) | — |

## プロジェクト構成

```
jamo/
├── jamo-core/      変換エンジン (Rust lib)
├── jamo-cli/       CLI / HTTP サーバー / 辞書ビルド (Rust bin)
├── jamo-ime/       Windows IME (TSF DLL, cdylib)
├── jamo-fcitx5/    Linux IME (fcitx5 アドオン, C++)
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
| 変換 (短文) | 13 us |
| 変換 (中文) | 95 us |
| 変換 (長文) | 107 us |
| 変換 (56 文コーパス平均、mozc 辞書) | 57 us (本家 mozc_server 1246 us) |
| peak RSS (56 文変換後、mozc 辞書) | 8.5 MB (本家 25.2 MB) |
| 辞書ロード | 0.1 ms (mmap) |
| 予測変換 | 2-6 us |
| WASM バイナリ | 144 KB |
| 辞書バイナリ | 8.4 MB (IPAdic) / 19.3 MB (mozc) |

## ライセンス

MIT
