# マイルストーン & タスク

## 方針: 反復深化

各イテレーションで**薄く全体を通す**。完璧にしてから次に進まない。

```
Iter 0: 足場
  cargo build が通る → cargo test が通る → CI green

Iter 1: 針金
  ハードコード10語辞書 → Viterbi → CLI出力 → HTTP JSON → WASM動作
  全部ハリボテでいい。E2Eで「きょう」→「今日」が通ることだけ証明

Iter 2: 実データ
  IPAdic投入 → 辞書ビルド → 実際の日本語が変換できる → APIで返せる
  LOUDS trie はここで入れる

Iter 3: 運用
  CF DO → ユーザー隔離 → 学習

Iter 4: 品質
  精度改善 → パフォーマンス → API安定化 → 1.0
```

壊すのは当たり前。各イテレーションで前のを作り直してよい。

---

## Iter 0: 足場

完了条件: `cargo test` が通る空のワークスペース、CI green、フック動作

| # | タスク | 成果物 |
|---|--------|--------|
| T001 | Cargo ワークスペース | jamo-core (lib) + jamo-cli (bin) が build 通る |
| T002 | CI | GitHub Actions: test + clippy + fmt |
| T003 | gitフック | pre-commit が動作する |

---

## Iter 1: 針金 — 最薄のE2Eパス

完了条件: `curl localhost:8080/convert?q=きょう` → `{"result":[{"candidates":["今日"]}]}`

**全レイヤーをハリボテで貫通させる。** 辞書は10語ハードコード、LMはunigramだけ、UIは最低限。
目的は「パイプライン全体が繋がる」ことの証明。

| # | タスク | 成果物 |
|---|--------|--------|
| T010 | ハードコード辞書 | 10語。HashMap。テスト: common_prefix_search |
| T011 | ラティス + Viterbi | 最単純な実装。接続コストはハードコード。テスト: 「きょう」→「今日」 |
| T012 | `jamo convert` CLI | 引数→変換→stdout。テスト: E2E |
| T013 | `jamo serve` HTTP | axum, `GET /convert` だけ。テスト: curl で動く |
| T014 | WASM ビルド | wasm-pack build が通る。テスト: Node.js で変換が動く |

ここで得られる学び:
- Viterbi のデータ構造は正しいか
- WASM に載せるときの制約は何か
- API の返り値の形は使いやすいか

---

## Iter 2: 実データ — 本物の日本語が変換できる

完了条件: 「きょうはいいてんきですね」→「今日はいい天気ですね」

Iter 1 の骨格に実データを流し込む。辞書構造を差し替え、精度を上げる。
**Iter 1 の設計が合わなければ壊して作り直す。**

### 辞書パイプライン

| # | タスク | 成果物 |
|---|--------|--------|
| T020 | IPAdic パース | CSV → (読み, 表層形, 品詞ID, コスト) |
| T021 | 品詞接続コスト行列 | matrix.def → i16 行列 |
| T022 | bigram LM | コスト算出・量子化 (8bit) |
| T023 | 辞書バイナリ | 全データを1ファイルにパック。`jamo dict build` |

### 省メモリ辞書

| # | タスク | 成果物 |
|---|--------|--------|
| T024 | ビットベクトル rank/select | テスト: 既知ビット列の rank/select |
| T025 | LOUDS trie | 構築 + 共通接頭辞検索 |
| T026 | HashMap → LOUDS 差し替え | 既存テストが全て通る |

### 変換精度

| # | タスク | 成果物 |
|---|--------|--------|
| T027 | 連文節変換 | 文節区切り。テスト: 複数文節の変換 |
| T028 | 文節伸縮 | 境界の手動調整。テスト: 伸縮→再変換 |
| T029 | N-best | 上位N件。テスト: 2番目以降の候補 |
| T030 | 変換精度テスト | 100文のテストコーパスで検証 |

### CLI/API更新

| # | タスク | 成果物 |
|---|--------|--------|
| T031 | `jamo dict info` | 辞書統計表示 |
| T032 | API: `/candidates`, 文節伸縮対応 | Iter 1 の API を拡充 |
| T033 | WASM更新 | 実辞書を埋め込んだ WASM ビルド |

---

## Iter 3: 運用 — 実際にユーザーが使える

完了条件: CF DO上で2人のユーザーが独立した辞書で変換できる

### デプロイ

| # | タスク | 成果物 |
|---|--------|--------|
| T041 | CF Worker | ルーター + JWT認証 |
| T042 | Durable Object | WASM統合 + SQLiteユーザー辞書 + hibernate |
| T043 | WebSocket | リアルタイム変換 |

### 学習

| # | タスク | 成果物 |
|---|--------|--------|
| T044 | 学習テーブル | LRU、commit時に更新、次回変換に反映 |
| T045 | ユーザー辞書 API | `/dict/add`, `/dict/remove`, `/dict/list`, `/dict/import` |
| T046 | 予測変換 | `/predict` |
| T047 | ドメイン辞書切り替え | `?domain=medical` |

### C API

| # | タスク | 成果物 |
|---|--------|--------|
| T048 | cbindgen | `#[no_mangle] extern "C"` + ヘッダ自動生成 |

---

## Iter 4: 品質 — 1.0 に向けた磨き上げ

完了条件: 「これが正解」と確信できる

| # | タスク | 成果物 |
|---|--------|--------|
| T050 | ベンチマークスイート | 変換レイテンシ、メモリ使用量、辞書サイズ |
| T051 | 精度評価基盤 | テストコーパスでの正解率計測 |
| T052 | NEologd 統合 | 新語辞書ビルド |
| T053 | API 仕様確定 | OpenAPI spec |
| T054 | 辞書フォーマット確定 | バージョニング付き仕様書 |
| T055 | ドキュメント | README, APIガイド, 辞書ビルドガイド |
| T056 | 1.0 リリース | タグ、crates.io、Docker Hub |

### Backlog (1.0後)

| # | タスク |
|---|--------|
| T060 | trigram LM |
| T061 | NN リランキング |
| T062 | 長文脈変換 |
| T063 | マルチスレッドパイプライン |
| T064 | 組み込みプロファイル検証 (Kindle/Raspi Zero) |

---

## Iter 5: Windows IME

完了条件: Windows のメモ帳でローマ字入力→ひらがな→漢字変換ができる

| # | タスク | 成果物 |
|---|--------|--------|
| T070 | ローマ字エンジン | romaji.rs (jamo-core) |
| T071 | jamo-ime crate 骨格 | Cargo.toml, DllMain, COM exports |
| T072 | COM/TSF 登録 | registry.rs |
| T073 | IClassFactory | class_factory.rs |
| T074 | TextInputProcessor | text_service.rs (Activate/Deactivate) |
| T075 | KeyEventSink | OnTestKeyDown/OnKeyDown |
| T076 | EditSession | edit_session.rs |
| T077 | Composition 状態マシン | composition.rs |
| T078 | ひらがな Composition | ローマ字→ひらがな下線表示 |
| T079 | DisplayAttribute | 3種下線スタイル |
| T080 | かな漢字変換統合 | engine.rs + Converting 状態 |
| T081 | 候補選択 | Space/↓/←/→ |
| T082 | 文節伸縮 | Shift+←/→ |
| T083 | 候補ウィンドウ | candidate_ui.rs (OS標準UI) |
| T084 | GHA Windows CI | windows-latest ビルドジョブ |

### 依存関係

```
T070 → T077 → T078
T071 → T072 → T073 → T074 → T075 → T076
T076 + T078 → T079
T079 + T080 → T081 → T082
T081 → T083
T071 → T084
```

| T085 | IME マルチプロセス化 | IPC プロトコル + Named Pipe サーバー + DLL プロキシ化 |
| T086 | デプロイスクリプト | scripts/deploy-ime.ps1 |

### 依存関係 (追加)

```
T085: T084 完了後
T086: 独立
```

---

## Iter 6: 辞書改善

完了条件: naist-jdic または EUC-JP の IPAdic から辞書ビルドが成功する

| # | タスク | 成果物 |
|---|--------|--------|
| T090 | naist-jdic 対応 | README に naist-jdic での辞書ビルド手順を記載、動作確認 |
| T091 | 多エンコード対応 | EUC-JP/Shift_JIS の CSV を自動判定して UTF-8 変換、encoding_rs 依存追加 |

### 依存関係

T090, T091 は独立 (どちらか片方だけでもよい)

---

## Iter 7: コードレビュー是正

完了条件: 2026-06-26 の分散コードレビューで見つかった High/Medium 指摘を潰し、再発防止テストを追加する

### 実装決定

- T100: 未知語はエラーにしない。ラティス構築時に全文字位置へ「1文字未知語ノード」を追加する。surface/yomi は入力1文字、left_id/right_id は 0、pos_type は POS_CONTENT、word_cost は既知語より十分悪い固定値にする。辞書一致がある区間では既知語が優先され、未知語だけの入力も元文字列を保持して返ること。
- T101: `boundaries` は byte offset のまま維持する。ただし core に共通 validator を置き、昇順、重複なし、全値が `input` の UTF-8 文字境界、範囲内、最後が `input.len()` を満たさない場合は slice しない。HTTP は 400、IPC は `IpcResponse::Error`、WASM は `JsError`。
- T102: 既存の `/convert` レスポンス互換性は壊さず、トップレベルに `boundaries: number[]` を追加する。`/resize` も `boundaries` を文字列配列ではなく `number[]` で返す。OpenAPI と README の例を同時に更新する。
- T104: `.dict` は読み込み時に `InvalidData` で落とす。最低限、`bits_len <= blocks.len() * 64`、`labels.len() == terminal.len()`、LOUDS の node 数整合、terminal range が `entries` 範囲内、`num_pos.checked_mul(num_pos)` 成功、`num_pos <= 8192` を検証する。
- T105: Viterbi の累積コストは `i64` に寄せる。公開 result の候補順だけを維持し、辞書エントリの on-disk cost 型は変えない。
- T110: Worker 認証は HMAC JWT に決める。`JWT_SECRET` で HS256 を検証し、`sub` を Durable Object name に使う。`?user=` は `ALLOW_DEV_USER=true` のときだけ許可し、OpenAPI では開発用として明記する。
- T114: CORS は default deny。`ALLOWED_ORIGINS` に完全一致した Origin だけ `Access-Control-Allow-Origin` を返し、OPTIONS は許可 Origin なら 204、不許可なら 403。
- T120: TSF composition は `Rc<RefCell<Option<ITfComposition>>>` で共有し、edit session が service 本体と同じ cell を更新する。`RefCell` 自体の clone は禁止。
- T121: `EndComposition` は空文字でも必ず `SetText` してから `EndComposition` する。Escape は空文字で cancel、IME off/Deactivate は現在の表示文字列を commit する。
- T122: COM lifetime は DLL 内の全 COM object に guard を持たせ、生成時に active count increment、Drop で decrement する。`DllCanUnloadNow` は lock count と active count の両方が 0 のときだけ `S_OK`。
- T126: Windows CI では build/test に加えて `scripts/ime-smoke.ps1` を追加し、DLL export、`regsvr32 /s` 登録/解除、`jamo serve --ime` 起動確認まで自動化する。メモ帳での手動入力確認は checklist として残す。

### Core / CLI

| # | タスク | 成果物 |
|---|--------|--------|
| T100 | 未知語フォールバック | 1文字未知語ノードで辞書未登録入力を保持する。テスト: 未知語のみ/既知語混在 |
| T101 | resize 境界検証 | `boundaries` の昇順・範囲内・UTF-8境界・末尾 `input.len()` を検証し、HTTP/IPC/WASMで 400/Error にする |
| T102 | resize API round-trip | `/convert` が文節境界を返し、`/resize` の `boundaries` は数値配列で返す。OpenAPI と実装を一致 |
| T103 | CLI HTTP API 補完 | `/predict` 実装、HTTP method 検証、malformed JSON/parameter を 4xx にする |
| T104 | 辞書バイナリ堅牢化 | `.dict` の LOUDS/terminal/entry/matrix 整合性検証、巨大 allocation/overflow 防止、破損辞書テスト |
| T105 | Viterbi コスト overflow 対策 | 累積コストを `i64` に変更し、長文・極端コストの回帰テスト |

### Cloudflare Worker

| # | タスク | 成果物 |
|---|--------|--------|
| T110 | Worker 認証 | HS256 JWT を `JWT_SECRET` で検証し、`sub` を user_id にする。dev query auth は明示フラグ時のみ |
| T111 | Worker deploy 手順修正 | `build-wasm` を deploy 前提に組み込み、README の R2 upload bucket/path を `jamo-dicts` と実作業ディレクトリに合わせる |
| T112 | Worker 辞書ロード retry | R2 miss/一時失敗で rejected promise を永続キャッシュしない。再試行・エラー分類テスト |
| T113 | WebSocket state 操作分離 | `commit`/`dict.*`/`dict.list` は wasm 辞書ロードなしで動くようにする |
| T114 | Worker API 契約整合 | method 検証、default deny CORS、許可 Origin の OPTIONS 204 を実装し、OpenAPI と一致 |
| T115 | `/dict/import` 件数修正 | `INSERT OR IGNORE` の実挿入件数だけ `added` に数える |

### Windows IME

| # | タスク | 成果物 |
|---|--------|--------|
| T120 | TSF composition 保持修正 | `Rc<RefCell<Option<ITfComposition>>>` で service/edit session が同じ cell を共有 |
| T121 | 空文字 EndComposition 修正 | 空文字でも `SetText` して終了。Escape は cancel、IME off/Deactivate は表示文字列を commit |
| T122 | COM lifetime 管理 | active object 参照を `DllCanUnloadNow` に反映し、参照中 DLL unload を防ぐ |
| T123 | モード切替/Deactivate cleanup | composition 中の IME off/Deactivate で現在の表示文字列を commit して active composition を終了 |
| T124 | 文節伸縮の実配線 | Shift+←/→ で `ImeEngine::resize` を呼び、結果・境界・候補 index を更新 |
| T125 | Candidate UI flags 修正 | `GetUpdatedFlags` と `GetDocumentMgr`/candidate string 更新通知の整合を取る |
| T126 | Windows 実行時 smoke test | `scripts/ime-smoke.ps1` で DLL export、登録/解除、`serve --ime` 起動確認を CI 化 |

---

## 依存関係

```
Iter 0: T001 → T002 → T003

Iter 1: T010 → T011 → T012 → T013
                         └──→ T014

Iter 2: T020 → T023 → T026 → T027 → T030
        T021 ↗        ↑
        T022 ↗    T024 → T025
                  T027 → T028, T029
                  T030 → T031, T032, T033

Iter 3: T033 → T041 → T042 → T043
        T027 → T044 → T045, T046
               T042 → T045
                       T047
               T048

Iter 4: T050 → T051 → T052 → T053 → T054 → T055 → T056

Iter 5: T070 → T077 → T078 → T079 → T081 → T082 → T083
        T071 → T072 → T073 → T074 → T075 → T076
        T076 + T078 → T079
        T079 + T080 → T081
        T071 → T084
```

## 反復深化のルール

1. **各Iterは全レイヤーを薄く通す** — エンジン・CLI・API・WASMが毎回繋がる
2. **前のIterの設計を壊してよい** — done.md に「なぜ壊したか」を残す
3. **Iter内はトップダウン** — 結合テストから書き始めて、足りないものを作る
4. **完璧にしてから次に進まない** — 「動く」を確認したら次のIterへ
5. **学びを優先** — 「これは違った」と気づくことが最大の成果
