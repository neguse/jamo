# 完了ログ

<!--
エントリ形式:
### タスク名 ✓ (YYYY-MM-DD)
- やったこと: 変更内容
- 変更したファイル: ファイル一覧
- よかったこと: 再利用できる知見
- 判断: 設計判断、却下した代替案
- 残課題: 未完了の作業
-->

### T001: Cargo ワークスペース初期化 ✓ (2026-02-24)
- やったこと: jamo-core (lib) + jamo-cli (bin) ワークスペース作成。dict/lattice/viterbi モジュール骨格。PRD.md, CLAUDE.md, doc/, .githooks/ 初期化。
- 変更したファイル: Cargo.toml, jamo-core/**, jamo-cli/**, PRD.md, CLAUDE.md, doc/*, .githooks/pre-commit, .gitignore
- よかったこと: モジュールを最初から分けておくとテストが書きやすい
- 判断: edition = 2024 を採用。jamo-cli は jamo-core に依存する形にした
- 残課題: CI未設定、gitフック未有効化

### T002+T003: CI + gitフック ✓ (2026-02-24)
- やったこと: GitHub Actions CI (test + clippy + fmt)。git hooks path を .githooks に設定。
- 変更したファイル: .github/workflows/ci.yml, doc/done.md
- よかったこと: dtolnay/rust-toolchain + Swatinem/rust-cache の組み合わせが楽
- 判断: CI は ubuntu-latest のみ。Windows/macOS は需要が出てから追加
- 残課題: pre-commit フックの動作確認は Iter 1 で実コミット時に検証

### T010: ハードコード辞書 ✓ (2026-02-24)
- やったこと: Dictionary トレイト + HashMapDict 実装。test_dict() で10語のテスト辞書。common_prefix_search の7テスト。
- 変更したファイル: jamo-core/src/dict.rs
- よかったこと: char_indices() でUTF-8バイト境界を安全に扱える。ひらがなは1文字3バイト。
- 判断: common_prefix_search の戻り値を (バイト長, Vec<Entry>) にした。バイト長はラティスのノード位置に直接使える。
- 残課題: 品詞IDは仮の値。接続コスト行列はまだない。

### T011: ラティス + Viterbi デコーダ ✓ (2026-02-24)
- やったこと: Lattice::build (辞書→ラティス構築、BOS/EOS付き)。Viterbi decode (前進+後退で最適パス)。SimpleConnectionCost (仮の固定値)。トップレベル convert() 関数。16テスト。
- 変更したファイル: jamo-core/src/lattice.rs, jamo-core/src/viterbi.rs, jamo-core/src/lib.rs
- よかったこと: ends_at[byte_pos] でノードを管理する設計がシンプルで正しく動いた。BOS/EOSを通常ノードと同じ構造にしたので特殊処理が少ない。
- 判断: 接続コストは Iter 1 では固定0。品詞接続で「天気」vs「天+木」の問題が出るが、Iter 2 で対応。Lattice を mutable で渡す設計は Viterbi のコスト書き込みのため。
- 残課題: 接続コスト行列なし。N-best なし。文節伸縮なし。

### T012: jamo convert CLI ✓ (2026-02-24)
- やったこと: `jamo convert <text>` サブコマンド実装。CLI結合テスト3件。
- 変更したファイル: jamo-cli/src/main.rs, jamo-cli/tests/cli_test.rs
- よかったこと: env!("CARGO_BIN_EXE_jamo-cli") でバイナリパスを取得してサブプロセステストが簡潔に書ける。
- 判断: clap は入れずに std::env::args で十分。Iter 1 では最小限。
- 残課題: サブコマンド体系 (dict, serve) は後続タスク。

### T013: jamo serve HTTP API ✓ (2026-02-24)
- やったこと: axum で `GET /convert` + `GET /health`。`jamo serve --port N` サブコマンド。HTTP結合テスト2件 (curl + サブプロセス)。
- 変更したファイル: jamo-cli/src/main.rs, jamo-cli/src/serve.rs, jamo-cli/tests/serve_test.rs, jamo-cli/Cargo.toml
- よかったこと: reqwest の代わりに curl サブプロセスでテストしたらビルド時間が大幅に短縮。ServerGuard の Drop でサーバー確実停止。
- 判断: reqwest を dev-dependency に入れると依存が爆発するので curl でテスト。テストポートは18091,18092で分けて並列実行可能に。
- 残課題: N-best 未対応。ユーザー辞書API未実装。辞書はリクエストごとに生成している (共有化が必要)。

### T020: IPAdic パース ✓ (2026-02-24)
- やったこと: IpaDict 実装。IPAdic CSV (EUC-JP→UTF-8) + matrix.def のパース。カタカナ→ひらがな変換。Dictionary + ConnectionCost トレイト実装。convert_with_conn() 追加。21テスト。
- 変更したファイル: jamo-core/src/ipadic.rs, jamo-core/src/lib.rs, .gitignore
- よかったこと: 接続コスト行列を入れただけで「天木」→「天気」に改善された。IPAdicのmatrix.defは1316x1316=173万エントリだが読み込みは3秒弱。
- 判断: IPAdic のCSVを直接読む方式。辞書バイナリ化はT023で。EUC-JP→UTF-8変換は事前にiconvで実施（Rust側ではUTF-8前提）。data/ipadic/ は .gitignore。
- 残課題: 「今日」vs「教」の精度問題。辞書ロード3秒は遅い（バイナリ化が必要）。LOUDS trie 未実装。

### T021: CLI/HTTP を IPAdic に接続 ✓ (2026-02-24)
- やったこと: CLI と HTTP サーバーを IPAdic 辞書に接続。--dict オプション追加。Arc<IpaDict> でサーバー全体で辞書共有。CLI結合テスト4件、HTTP結合テスト2件を IPAdic 対応に更新。
- 変更したファイル: jamo-cli/src/main.rs, jamo-cli/src/serve.rs, jamo-cli/tests/cli_test.rs, jamo-cli/tests/serve_test.rs
- よかったこと: IPAdic + 接続コスト行列だけで「私は学生です」「東京に行きたい」「日本語を勉強する」が正しく変換された。Arc<IpaDict> でリクエスト間の辞書共有が簡潔に実現。
- 判断: load_dict() でデフォルトパス候補 (data/ipadic, ../data/ipadic) を順に試す設計。テストは IPAdic がない環境ではスキップ。curl サブプロセスでの HTTP テストを維持。
- 残課題: 「きょう」→「教」の精度問題。N-best 未対応。辞書バイナリ化未着手。

### T024: ビットベクトル rank/select ✓ (2026-02-24)
- やったこと: 簡潔ビットベクトル (BitVec) 実装。rank1/rank0/select1/select0 を O(1) で提供。大ブロック (256bit) + 小ブロック (64bit) + popcount 方式。11テスト。
- 変更したファイル: jamo-core/src/bitvec.rs, jamo-core/src/lib.rs
- よかったこと: from_bits() と from_raw() の2つのコンストラクタでテストと実用の両方が楽。rank/select の整合性テストで正しさを網羅的に検証できた。
- 判断: select は線形探索 (大ブロック内4ブロック)。LOUDS trie の用途では十分。BMI2 の PDEP/PEXT は WASM で使えないので不採用。
- 残課題: LOUDS trie (T025) で実際に使う。

### T025: LOUDS trie ✓ (2026-02-24)
- やったこと: LOUDS trie 実装。バイト単位の trie で UTF-8 文字列の共通接頭辞検索を行う。中間 trie を BFS で LOUDS ビット列に変換。BitVec の rank0/select0 でナビゲーション。10テスト。
- 変更したファイル: jamo-core/src/louds.rs, jamo-core/src/lib.rs
- よかったこと: node_bit_start(node) = select0(node-1)+1 と node_from_bit(pos) = rank1(pos)+1 の2関数で LOUDS ナビゲーションが完結する。バイト単位 trie なので UTF-8 を意識せず動作。
- 判断: ラベルは u8 (バイト値) で、ひらがな1文字 (3バイト) は trie の3段に展開される。エントリは別プールに持ち、terminal に (start, end) インデックスを格納。find_child は子の線形探索だが、ひらがなは0xe3始まりの3バイトなので分岐は少ない。
- 残課題: IpaDict への組み込み (T026)。

### T026: HashMap → LOUDS trie 差し替え ✓ (2026-02-24)
- やったこと: IpaDict の内部辞書を HashMap<String, Vec<Entry>> から LoudsTrie に差し替え。Dictionary トレイト実装は trie.common_prefix_search() に委譲。vocab_size/reading_count はロード時に計算して保持。全47テスト通過。
- 変更したファイル: jamo-core/src/ipadic.rs
- よかったこと: 差し替えがスムーズ。common_prefix_search の返り値型が同じなので Dictionary トレイト実装は1行で済んだ。IPAdic + 接続コストの変換結果も変化なし。
- 判断: HashMap をソートして LOUDS trie を構築。ロード時間はソート+構築分増えるが、検索時のメモリ効率が大幅改善。ロード時間改善は辞書バイナリ化 (T023) で対応。
- 残課題: メモリ使用量の計測 (HashMap 比較)。

### T023: 辞書バイナリフォーマット ✓ (2026-02-24)
- やったこと: LOUDS trie + 接続コスト行列のバイナリシリアライズ/デシリアライズ。`jamo dict build` サブコマンド。load_dict() でバイナリ辞書を優先ロード。roundtrip テスト。
- 変更したファイル: jamo-core/src/ipadic.rs, jamo-core/src/bitvec.rs, jamo-core/src/louds.rs, jamo-cli/src/main.rs
- よかったこと: CSV ロード 3.76秒 → バイナリロード 0.19秒 (約20倍高速化)。バイナリサイズ 18.8MB。BitVec は blocks+len だけ保存して rank テーブルは再構築。
- 判断: リトルエンディアン固定。magic "JAMO" + version 1 のヘッダ。terminal は u32::MAX をセンチネルに。エントリの surface は可変長 (u16 length prefix)。mmap は将来課題。
- 残課題: バイナリサイズ 18.8MB は大きい (接続コスト行列だけで 3.5MB)。圧縮やエントリのデデュプは後のIter。

### T029: N-best 候補 ✓ (2026-02-24)
- やったこと: Segment に candidates フィールド追加。decode_nbest() で Viterbi 最適パスの各セグメントに対し、同区間の全候補を word_cost 順に返す。convert_nbest() トップレベル API。HTTP API の n パラメータが実際に機能するように更新。7テスト (既存4+新規3)。
- 変更したファイル: jamo-core/src/viterbi.rs, jamo-core/src/lib.rs, jamo-cli/src/serve.rs
- よかったこと: Viterbi の forward pass はそのまま。候補収集はラティスの ends_at[end] を走査するだけで追加コスト最小。API の `candidates: vec![s.surface.clone()]` を `s.candidates.clone()` に変えるだけで HTTP 側も完了。
- 判断: 「同じ区間の候補」方式 (各セグメントの読みが同じ他の表層形を列挙)。フルパス N-best (異なるセグメンテーション) は複雑なので見送り。surface は Viterbi 選択のものを使い、candidates の先頭に配置。
- 残課題: フルパス N-best は未対応。

### T027: 連文節変換 ✓ (2026-02-24)
- やったこと: Entry に pos_type (Content/Function) 追加。IPAdic CSV の品詞フィールドから自立語/付属語を分類。Viterbi 結果を文節 (Clause) 単位にグルーピング。文節候補は各単語の代替候補を組み合わせて生成。HTTP API を文節単位のレスポンスに変更。バイナリフォーマット v2 (pos_type 追加)。6テスト新規 (classify_pos, clauses_kyouha, clauses_kyouha_tenki_desune, clauses_candidates, clauses_single_content, clauses_with_ipadic)。
- 変更したファイル: jamo-core/src/dict.rs, jamo-core/src/lattice.rs, jamo-core/src/viterbi.rs, jamo-core/src/ipadic.rs, jamo-core/src/louds.rs, jamo-core/src/lib.rs, jamo-cli/src/serve.rs
- よかったこと: 品詞分類のルールがシンプル (助詞/助動詞/非自立→Function, それ以外→Content)。group_clauses() はセグメント列の後処理で、Viterbi 本体に手を入れずに済んだ。
- 判断: 品詞タイプは2値 (Content/Function) で十分。接頭詞は Content 扱い (新文節開始)。文節候補は単語代替の組み合わせ方式 (再Viterbi方式は複雑なので見送り)。バイナリ v2 で後方互換を切った (pre-1.0)。
- 残課題: 文節伸縮 (T028) 未実装。フルパス N-best による文節候補は未対応。

### T028: 文節伸縮 ✓ (2026-02-24)
- やったこと: resize_boundaries() で文節境界を文字単位で伸縮。convert_with_boundaries() で文節境界を指定して変換。HTTP API に GET /resize エンドポイント追加。伸ばして次の文節がなくなる場合は統合。5テスト新規 (resize_extend, resize_shrink, resize_boundary_limits, convert_with_boundaries, resize_and_reconvert)。
- 変更したファイル: jamo-core/src/viterbi.rs, jamo-core/src/lib.rs, jamo-cli/src/serve.rs
- よかったこと: boundaries (バイト位置配列) をクライアントとやり取りする設計がシンプル。各文節を独立に Viterbi 変換するのでパフォーマンスもよい。
- 判断: 文字単位の伸縮 (ひらがな1文字=3バイト)。最後の文節は伸縮不可 (入力末尾に固定)。文節が完全に吸収される場合は統合 (boundary を remove)。
- 残課題: フルパス N-best 未対応。

### T030: 精度テスト ✓ (2026-02-24)
- やったこと: 52文のテストコーパス作成。文一致率・文字精度・Top-N の3指標を計測。回帰検出の閾値設定。jamo-core/tests/accuracy_test.rs に2テスト。
- 変更したファイル: jamo-core/tests/accuracy_test.rs
- よかったこと: ベースライン計測で改善すべき点が明確になった。短い単語 (2-3文字) が特に弱い。文としては半分弱が正解。
- 判断: 文一致率 48.1%、文字精度 69.1% (IPAdic + Viterbi, bigram LM なし)。閾値は文一致率 40%、文字精度 60% (回帰検出用)。短い単語の精度が低いのは接続コスト行列だけでは文脈が不足するため。
- 残課題: bigram LM (T022) で精度改善。コーパス拡充。

### T041/T042: CF Worker + Durable Object ✓ (2026-02-24)
- やったこと: jamo-core に wasm feature 追加 (wasm-bindgen, serde, serde_json)。JamoEngine WASM バインディング (convert, resize)。jamo-worker/ プロジェクト新規作成: CF Worker ルーター + Durable Object (WASM エンジン + R2 辞書ロード)。
- 変更したファイル: jamo-core/Cargo.toml, jamo-core/src/lib.rs, jamo-core/src/ipadic.rs, jamo-core/src/wasm.rs (新規), jamo-worker/ (新規: index.ts, do.ts, wrangler.toml, tsconfig.json, package.json, wasm.d.ts)
- よかったこと: wasm-pack --target web で 144KB の WASM バイナリ。wrangler dry-run で 157KB バンドル。CF Workers から .wasm を直接 import して init() に渡す方式が動作。
- 判断: Durable Object でユーザーごとにエンジンインスタンスを分離。辞書は R2 バケットに格納し、DO 初回リクエスト時にロード (lazy init)。認証は簡易実装 (Bearer トークン = user_id)。
- 残課題: wrangler deploy (R2バケット作成+辞書アップロード要)。SQLite ユーザー辞書 (T044)。

### T043: WebSocket ✓ (2026-02-24)
- やったこと: Durable Object に WebSocket エンドポイント (/ws) 追加。state.acceptWebSocket() で CF の Hibernation API 対応。JSON メッセージで convert/resize をリアルタイム処理。リクエスト ID で応答を対応付け。
- 変更したファイル: jamo-worker/src/do.ts
- よかったこと: Durable Object の webSocketMessage/webSocketClose ハンドラで WebSocket ライフサイクルが簡潔に実装できた。HTTP と WebSocket で同じ ensureEngine() を共有。
- 判断: メッセージ形式は `{ type: "convert"|"resize", id?: string, ... }`。id フィールドでリクエスト・レスポンスの対応付け。バイナリメッセージは非サポート (テキスト JSON のみ)。
- 残課題: 再接続時の状態復元。heartbeat/ping-pong。

### T044: 学習テーブル ✓ (2026-02-24)
- やったこと: DO の SQLite ストレージに user_history テーブル追加。ユーザーの変換確定 (commit) を記録し、次回変換時に候補順位を調整。LRU で上限 10,000 エントリ。HTTP POST /commit + WebSocket commit メッセージ対応。
- 変更したファイル: jamo-worker/src/do.ts, jamo-worker/wrangler.toml
- よかったこと: `new_sqlite_classes` で SQLite バックエンドに切り替えるだけで sql.exec() が使える。候補の reorder は TypeScript 側で WASM 結果を後処理する方式がシンプル。
- 判断: 学習データは WASM エンジンに渡さず TypeScript 側で候補の並べ替えのみ。スキーマは (yomi, surface) を主キーに count + last_used で LRU 管理。ON CONFLICT で UPSERT。
- 残課題: 学習データのエクスポート/インポート。

### T045: ユーザー辞書 API ✓ (2026-02-24)
- やったこと: SQLite user_dict テーブル追加。HTTP API: POST /dict/add, /dict/remove, GET /dict/list, POST /dict/import。WebSocket: dict.add, dict.remove, dict.list メッセージ。変換時にユーザー辞書の候補を注入 (injectUserDict)。
- 変更したファイル: jamo-worker/src/do.ts
- よかったこと: ユーザー辞書の候補は candidates の先頭に unshift するだけで優先表示。学習テーブルの reorder と組み合わせて、ユーザー登録単語は常に上位に来る。
- 判断: user_dict は (yomi, surface) を PK にシンプルな構造。import はバッチ INSERT。WASM エンジンに注入するのではなく、TypeScript 側で候補リストに追加する方式。
- 残課題: ユーザー辞書のエクスポート。品詞指定の対応。

### T046: 予測変換 ✓ (2026-02-24)
- やったこと: LOUDS trie に prefix_search (DFS でサブツリー列挙) 追加。IpaDict.prefix_search() 公開。WASM に predict() メソッド追加。DO に /predict エンドポイント + WebSocket predict メッセージ。ユーザー辞書・履歴からも LIKE prefix% で予測候補追加。
- 変更したファイル: jamo-core/src/louds.rs, jamo-core/src/ipadic.rs, jamo-core/src/wasm.rs, jamo-worker/src/do.ts
- よかったこと: LOUDS trie の構造を活かしてプレフィクスノードまで辿り→DFS で子孫を列挙する方式がシンプル。重複除去は HashSet で O(n)。ユーザー辞書/履歴の候補をシステム辞書候補と統合。
- 判断: limit パラメータで結果数を制限 (DFS 打ち切り)。ユーザー候補をシステム候補より優先 (先頭に配置)。cost=0 でユーザー候補を返す (実コストはユーザー辞書にないため)。
- 残課題: 予測候補のスコアリング改善 (頻度・文脈)。

### T048: C API (cbindgen) ✓ (2026-02-24)
- やったこと: capi.rs に `extern "C"` 関数群を実装。cbindgen で jamo.h 自動生成。jamo_engine_open/free, jamo_convert, jamo_predict, jamo_string_free の5関数。JSON 文字列で結果を返す方式。
- 変更したファイル: jamo-core/src/capi.rs (新規), jamo-core/src/lib.rs, jamo-core/cbindgen.toml (新規), jamo-core/include/jamo.h (新規)
- よかったこと: cbindgen が不透明型 (opaque struct) と関数シグネチャを自動生成。手動の JSON 構築で serde_json 依存を避けた。cdylib で 497KB の共有ライブラリ。
- 判断: WASM の JamoEngine と名前衝突を避けるため C API 側は JamoCEngine。JSON で結果を返す方式は言語非依存で扱いやすい。safety doc は全 unsafe 関数に記載。
- 残課題: resize の C API 追加。エラー情報の取得 API。

### T047: ドメイン辞書切り替え ✓ (2026-02-24)
- やったこと: `?domain=X` パラメータで辞書を切り替え。R2 から `{domain}.dict` をロード。ドメインごとにエンジンをキャッシュ (Map)。WASM 初期化は1回のみ。HTTP/WebSocket 全エンドポイント対応。
- 変更したファイル: jamo-worker/src/do.ts
- よかったこと: Map<domain, WasmEngine> でドメインごとにキャッシュする設計がシンプル。WASM 初期化は1回で済み、辞書のみドメインごとにロード。
- 判断: デフォルトは "ipadic"。R2 のキー命名は `{domain}.dict`。ドメイン辞書の作成自体はユーザー側の責任 (jamo dict build + R2 アップロード)。
- 残課題: ドメイン辞書の管理 API (一覧・削除)。

### T050: ベンチマークスイート ✓ (2026-02-24)
- やったこと: criterion ベンチマーク。convert (short/medium/long/complex)、dict_load、prefix_search (1/2/3char) の3グループ。
- 変更したファイル: jamo-core/Cargo.toml, jamo-core/benches/convert_bench.rs (新規)
- よかったこと: 変換レイテンシ: 短文10µs, 中文87µs, 長文99µs, 複雑123µs。prefix_search 1-2µs。辞書ロード 64ms。十分なパフォーマンス。
- 判断: criterion の html_reports feature で詳細なレポート出力。辞書が見つからない場合はスキップ。
- 残課題: メモリ使用量計測。regression detection (CI統合)。

### T051: 精度評価基盤 ✓ (2026-02-24)
- やったこと: T030 の精度テストをカテゴリ別評価に拡充。10カテゴリ (NounPhrase, Adjective, VerbConjugation, Polite, Daily, Place, Time, Complex, ShortVerb, ShortNoun) に分類。カテゴリ別の正解率を出力。
- 変更したファイル: jamo-core/tests/accuracy_test.rs
- よかったこと: カテゴリ別精度で弱点が明確に: VerbConjugation/Place/Time は 100%、Adjective 0%、ShortNoun 30%。カタカナ変換の問題が主要な失敗パターン。
- 判断: Category enum で構造化。テストケースを TestCase struct に統一。既存の回帰閾値 (文一致≥40%, 文字精度≥60%) は維持。
- 残課題: カテゴリ別の回帰検出。コーパス拡充。

### T053: API 仕様確定 ✓ (2026-02-24)
- やったこと: OpenAPI 3.0.3 仕様書を doc/api.yaml に作成。全エンドポイント (convert, resize, predict, commit, dict/add, dict/remove, dict/list, dict/import, ws, health) を定義。スキーマ (ConvertResponse, PredictResponse)、認証 (Bearer, query param) も記載。
- 変更したファイル: doc/api.yaml (新規)
- よかったこと: 全エンドポイントを一箇所にまとめたことで実装との乖離を検出しやすい。components/schemas でレスポンス型を共有。
- 判断: YAML 形式で OpenAPI spec を記述。WebSocket はパス定義+description でメッセージ形式を文書化。
- 残課題: バリデーションツール (swagger-cli validate) の CI 統合。

### T054: 辞書フォーマット確定 ✓ (2026-02-24)
- やったこと: doc/dict-format.md にバイナリ辞書フォーマットの仕様書を作成。Header, LOUDS BitVec, Labels, Terminal, Entries, Connection Matrix, Stats の各セクションのバイトレイアウトを詳細に記載。変更履歴 (v1→v2) も文書化。
- 変更したファイル: doc/dict-format.md (新規)
- よかったこと: ipadic.rs の write_binary/read_binary の実装と1:1で対応する仕様書。他言語での実装参照としても使える。
- 判断: Markdown + テーブルで記述。コードとの対応を明示するためセクション順を実装の書き込み順に合わせた。
- 残課題: フォーマットのバリデーションツール (magic + version チェック CLI)。

### T052: NEologd統合 — 辞書マージ機能 ✓ (2026-02-24)
- やったこと: IpaDict::load_merge() メソッドを追加。ベースIPAdicディレクトリ + 追加CSV (ファイル/ディレクトリ) をマージして辞書を構築。CLI に `jamo dict merge --base <dir> --extra <path>... -o <output>` コマンドを追加。ipadic.rs をリファクタリングし parse_csv_dir / build_from_entries ヘルパーを抽出。
- 変更したファイル: jamo-core/src/ipadic.rs, jamo-cli/src/main.rs
- よかったこと: NEologd等のCSVはIPAdicと同一形式なので、既存パーサーをそのまま再利用。matrix.defはベースを使用するため品詞IDの整合性が保たれる。load()も内部でparse_csv_dir/build_from_entriesを再利用し重複なし。
- 判断: merge専用の別メソッドにした。load()のシグネチャ変更は既存テストへの影響大。追加CSV側にmatrix.defは不要(ベースを使用)。
- 残課題: NEologd CSV の自動ダウンロード/ビルドスクリプト。

### T055: ドキュメント ✓ (2026-02-24)
- やったこと: README.md を作成。クイックスタート (ビルド・CLI変換・HTTPサーバー・辞書マージ)、HTTP APIリファレンス、WASM/C API/CF Workers デプロイ手順、プロジェクト構成、パフォーマンス表を記載。
- 変更したファイル: README.md (新規)
- よかったこと: 既存の doc/api.yaml, doc/dict-format.md への参照で詳細は委譲し、README は概要+クイックスタートに集中。
- 判断: README 1ファイルに集約。APIガイド・辞書ビルドガイドは README のセクションとして十分な粒度で記載し、別ファイルにはしない。
- 残課題: CONTRIBUTING.md (コントリビューションガイド)。

### T056: 1.0 リリース準備 ✓ (2026-02-24)
- やったこと: jamo-core/jamo-cli のバージョンを 0.1.0 → 1.0.0 に更新。Cargo.toml に crates.io 用メタデータ (description, license, repository, keywords, categories) を追加。MIT LICENSE ファイルを作成。doc/current.md を 1.0 最終状態に更新。
- 変更したファイル: jamo-core/Cargo.toml, jamo-cli/Cargo.toml, LICENSE (新規), doc/current.md, doc/done.md
- よかったこと: Iter 0〜4 の全タスクが完了。68テスト全通過の状態で 1.0 を迎えられた。
- 判断: crates.io publish と git tag は手動で行う (CI/CD 未構築のため)。Cargo.toml の準備のみ自動化。
- 残課題: `cargo publish`, `git tag v1.0.0`, GitHub Release 作成。

### T070-T084: Windows IME (Iter 5) ✓ (2026-02-24)
- やったこと: jamo-core にローマ字→ひらがな変換 FSM (romaji.rs) を追加。jamo-ime crate を新規作成し、Windows TSF COM DLL として IME 全体を実装。Platform 非依存の状態マシン (composition.rs) + Windows 専用 TSF コード (#[cfg(windows)] ゲート)。GHA に Windows CI ジョブ追加。
- 変更したファイル:
  - jamo-core/src/romaji.rs (新規), jamo-core/src/lib.rs
  - jamo-ime/ (新規crate): Cargo.toml, src/lib.rs, src/globals.rs, src/composition.rs, src/class_factory.rs, src/text_service.rs, src/edit_session.rs, src/display_attribute.rs, src/engine.rs, src/candidate_ui.rs, src/registry.rs
  - Cargo.toml (workspace members)
  - .github/workflows/ci.yml (Windows ジョブ追加)
  - doc/tasks.md, doc/current.md, doc/done.md
- よかったこと: composition.rs を Platform 非依存にした設計が良い。Linux 上で 16 テストを実行可能。romaji.rs も完全に Platform 非依存で 35 テスト。windows-rs 0.62 では implement マクロが windows-core 経由で自動的に使える (features に指定不要)。
- 判断: All-Rust (windows-rs) アプローチを採用。jamo-core を直接 Rust ライブラリとして呼び出すため JSON/C API 不要。ローマ字変換は MS-IME 方式 (nn は両方消費して「ん」を確定)。辞書なしのフォールバック (engine.rs の Option<IpaDict>) で起動だけはできる設計。
- 残課題: Windows 実機テスト (regsvr32 → メモ帳)。辞書同梱インストーラー。edit_session.rs の StartComposition で CompositionSink の受け渡しが todo!() のまま。

### T085+T086: IME マルチプロセス化 + デプロイスクリプト ✓ (2026-02-24)
- やったこと:
  - Phase A: `scripts/deploy-ime.ps1` を作成。DLL リネーム→コピー→explorer 再起動でデプロイ1コマンド化。
  - Phase B: IME をマルチプロセス方式に変更。TSF DLL を IPC クライアント化し、変換ロジックを `jamo serve --ime` 別プロセスに分離。
    - jamo-core に `ipc` feature + `ipc.rs` モジュール追加 (IPC プロトコル型定義、NDJSON形式)
    - ConvertResult/Clause/Segment に serde derives 追加 (ipc feature ゲート)
    - jamo-cli に `ime_server.rs` 追加 (Named Pipe (Windows) / Unix Socket (Linux) サーバー)
    - jamo-cli の `serve --ime` サブコマンド追加
    - jamo-ime の engine.rs を IPC クライアントに完全書き換え (辞書ロード削除、Named Pipe 接続)
    - IPC 未接続時はフォールバック (ひらがなをそのまま返す)、自動再接続機能付き
- 変更したファイル:
  - scripts/deploy-ime.ps1 (新規)
  - jamo-core/Cargo.toml, jamo-core/src/lib.rs, jamo-core/src/viterbi.rs, jamo-core/src/ipc.rs (新規)
  - jamo-cli/Cargo.toml, jamo-cli/src/main.rs, jamo-cli/src/ime_server.rs (新規)
  - jamo-ime/Cargo.toml, jamo-ime/src/engine.rs, jamo-ime/src/text_service.rs
  - doc/tasks.md, doc/current.md, doc/done.md
- よかったこと:
  - NDJSON プロトコルはシンプルで実装が容易。BufReader/read_line で十分。
  - `#[cfg_attr(feature = "ipc", derive(serde::Serialize, serde::Deserialize))]` で既存型を非侵入的に拡張できた。
  - IPC 型を jamo-core に置くことでクライアント/サーバー間の型不整合を排除。
  - DLL 側は RefCell<Option<IpcConnection>> で single-threaded COM と相性良い。
- 判断:
  - Named Pipe (Windows) + Unix Socket (Linux) を選択。TCP localhost も可能だが、Named Pipe はセキュリティ的に優れ (ACL 制御可)、パイプ名で識別が容易。
  - IPC プロトコルは JSON (NDJSON)。バイナリは速いが、デバッグ容易性と開発効率を優先。
  - DLL 側の辞書ロードを完全削除。サーバー側に辞書を集約することで DLL サイズ削減 + ロード時間短縮。
  - Mozc/azooKey 等の先行 IME と同じマルチプロセスパターンを採用。
- 残課題: Windows 実機テスト。IPC タイムアウト設定。サーバー自動起動 (DLL からサーバープロセスを起動)。

### C API 拡張: jamo_resize + jamo_romaji_* ✓ (2026-08-18)
- やったこと:
    - capi.rs に文節伸縮 `jamo_resize` を追加 (resize_boundaries → try_convert_with_boundaries を convert と同じ JSON で返す)
    - RomajiConverter の C API `jamo_romaji_new/free/feed/backspace/flush/pending/reset` と `JAMO_BACKSPACE_*` 定数を追加
    - convert の JSON 生成を `convert_result_to_json` に共通化
    - cbindgen 設定に JamoCRomaji (jamo_romaji_t) を追加しヘッダ再生成
    - capi テスト 5 件追加 (romaji 3 + 実辞書での convert/resize e2e 2)
- 変更したファイル: jamo-core/src/capi.rs, jamo-core/cbindgen.toml, jamo-core/include/jamo.h, doc/current.md, doc/done.md
- よかったこと: HTTP /resize と同じ流れ (resize → 再変換) を C API でも踏襲し、応答 JSON を convert と共通化できた
- 判断:
    - DM250 組み込み (lite-dm2x) は単一プロセスで使うため、IPC (ime_server) ではなく in-process C API を選択
    - romaji の状態は不透明ハンドルで保持 (グローバル状態を避け、複数インスタンス可)
    - boundaries はカンマ区切り文字列で受ける (HTTP API のパラメータ表現と揃え、C の配列受け渡しを避ける)
    - engine 系テストは ../data/ipadic.dict が無ければ skip (既存 test_convert_with_ipadic と同じ流儀)
- 残課題: 学習・ユーザー辞書は core 未実装 (DM250 は当面無学習で使う)

### T130 候補ランキング接続コスト算入 + Top-5 メトリクス実測化 ✓ (2026-08-18)
- やったこと:
    - decode_nbest の同区間候補の順位付けを word_cost 単独から文脈込みコスト (total_cost + 次パスノードへの接続コスト) に変更
    - 同一表層の品詞違いエントリが候補に重複して並ぶバグを修正 (表層で dedup してからスコア順ソート)
    - accuracy_test の Top-5 メトリクスを実測化 (従来は candidates を検査しておらず Top-1 と同値を報告していた)。「各文節の上位5候補から選んで期待文に到達できるか」を再帰で判定
    - Top-5 到達率の回帰閾値 (60%) を追加
- 変更したファイル: jamo-core/src/viterbi.rs, jamo-core/src/viterbi/tests.rs, jamo-core/tests/accuracy_test.rs, doc/current.md, doc/done.md
- よかったこと: Viterbi 前進パスで全ノードに計算済みの total_cost をそのまま候補スコアに流用でき、追加コストは接続行列引き1回のみ
- 判断:
    - 候補スコアは「BOS からそのノードまでの最良コスト + 次パスノードへの接続コスト」とした。前ノード固定の局所スコアより、上流の分割違いも織り込める
    - Top-1 (48.1%) はこの変更では動かない (最良パス自体は不変)。実測 Top-5 到達率は 75.0% → 78.8%
- 残課題: 到達不能 11 件 (かく→書く が候補圏外等) は単語コスト再推定 (T132) の領分

### T131 かな/カナ フォールバック候補 + 境界変換の候補生成修正 ✓ (2026-08-18)
- やったこと:
    - 文節候補列の末尾に読みのひらがな/カタカナを重複なしで常に追加 (group_clauses と try_convert_with_boundaries の両経路)
    - try_convert_with_boundaries の候補生成を修正: 複数の内部文節を含む境界区間で部分スパンの候補文字列がそのまま並んでいた (選ぶと文節の残りが消える)。内部文節1つを置換した全スパンのバリエーションに変更
    - テスト4件追加 (フォールバック2経路、重複なし、全スパン置換の回帰)
- 変更したファイル: jamo-core/src/viterbi.rs, jamo-core/src/viterbi/tests.rs, doc/current.md, doc/done.md
- よかったこと: フォールバック追加で境界変換経路の部分スパン混入バグが顕在化し、同時に修正できた
- 判断:
    - フォールバックは候補列の末尾 (最低優先)。カタカナ語の第1候補化は T132 (単語コスト再推定) の領分で、ここでは「必ず選べる」ことだけ保証する
    - カタカナ恒等表層へのペナルティ案は Iter 8 設計決定の通り却下済み (コーヒー/珈琲問題)
- 残課題: 52文コーパスでは Top-5 到達率は不変 (78.8%)。効果は辞書にない語・カタカナ語入力時の UX で、コーパスにそのケースを足すのは T132 と同時に行う

### T133 セッション型 C API v2 ✓ (2026-08-18)
- やったこと:
    - jamo-core/src/session.rs 新設: 入力→変換→候補選択→伸縮→確定の状態機械 (Session)。辞書は保持せず操作の引数で受け取る (状態とロジックの分離)。テスト16件
    - capi.rs を全面書き換え: JSON 返しのステートレス API (jamo_convert/jamo_resize/jamo_predict/jamo_romaji_*/jamo_string_free) を廃止し、jamo_session_* (21関数) に刷新。文字列は セッション内バッファ返しで呼び出し側の解放不要
    - エラーコード導入 (JAMO_OK/JAMO_ERR/JAMO_ERR_STATE/JAMO_ERR_INDEX/JAMO_ERR_RESIZE/JAMO_ERR_EMPTY)
    - cbindgen.toml 更新 + jamo.h 再生成、README / PRD の C API 節を v2 に同期
    - jamo-core を 2.0.0 に (破壊的変更のためメジャーバンプ)
- 変更したファイル: jamo-core/src/session.rs (新規), jamo-core/src/capi.rs, jamo-core/src/lib.rs, jamo-core/cbindgen.toml, jamo-core/include/jamo.h, jamo-core/Cargo.toml, README.md, PRD.md, doc/current.md, doc/done.md, doc/tasks.md
- よかったこと: lite-dm2x の ime.lua が自前実装していた状態管理 (state/kana/clauses/boundaries/focus/sel + 手書き JSON デコーダ75行) がセッションに丸ごと対応し、シム側の設計検証に使えた
- 判断:
    - モデル操作 API に徹し、キーイベント処理を入れない (キー方針は lite-dm2x の ime.lua、Windows の composition.rs などホスト側の責務)
    - 返却文字列の寿命は「同じセッションへの次の呼び出しまで」。Lua バインディング (lua_pushstring が即コピー) とも C 呼び出し側とも相性が良く、malloc/free 往復を排除できる
    - engine はセッション生成時に束縛 (毎回渡すより誤用しにくい)。寿命規約はヘッダの Safety に明記
    - JSON API は残さない (CLAUDE.md: 互換 shim を作らない。HTTP/WASM は Rust API 直叩きで無関係)
- 残課題: lite-dm2x 側の追随 (api_jamo.c / ime.lua / abi_smoke.c)。commit 学習 (T134) をセッションに載せる。jamo-ime (Windows) の composition.rs を core Session ベースに寄せる統合。capi.rs 607行 (YELLOW) の分割検討

### T134 commit 学習 ✓ (2026-08-18)
- やったこと:
    - jamo-core/src/learning.rs 新設: 読み→確定表層の LRU (LearningStore、容量1024)。TSV でファイル永続化。テスト7件
    - Session::convert/resize が Option<&LearningStore> を受け、学習済み表層を文節候補の先頭へ (候補になければ挿入)。commit(Option<&mut LearningStore>) で各文節の (読み→確定表層) を記録
    - C API: エンジンが RefCell<LearningStore> を保持し全セッションで共有。jamo_session_convert/resize/commit に自動配線。jamo_engine_save_user_data / jamo_engine_load_user_data を追加しヘッダ再生成
    - セッション学習テスト2件 + capi 学習フローのテスト1件 (別セッションへの学習反映と保存/読込)
- 変更したファイル: jamo-core/src/learning.rs (新規), jamo-core/src/session.rs, jamo-core/src/capi.rs, jamo-core/src/lib.rs, jamo-core/include/jamo.h, doc/current.md, doc/done.md
- よかったこと: Session が辞書非保持の設計だったので、学習も引数渡しで自然に入り core は入出力レスのまま保てた
- 判断:
    - 記録は確定した全文節 (選択変更の有無を問わず)。ユーザーが受け入れた変換は正例であり、LRU 上限があるので肥大しない
    - 学習表層が候補にない場合も先頭に挿入する (この読みで過去に確定した文字列なので提示してよい)
    - ストアはエンジン単位 (C API)。複数セッションで学習を共有し、永続化パスはホストが与える
    - 線形走査の Vec 実装 (容量1024で µs オーダー、HashMap+順序管理の複雑さに見合わない)
- 残課題: serve (HTTP/DO) への学習配線は別タスク (PRD の /commit)。jamo-ime (Windows) は IPC サーバー側への配線が必要

### 「省メモリ」看板を下ろし Iter 9 をバックログ化 ✓ (2026-08-19)
- やったこと: README の特徴から「省メモリ」を外し「小さいエンジン (WASM 144KB)」に置き換え。Iter 9 をバックログに変更し、tasks.md に状態と「完了が看板を掲げ直す条件」を記載。
- 変更したファイル: README.md, doc/tasks.md, doc/current.md, doc/done.md
- よかったこと: 実測基盤 (bench-memory.py / dict info / estimate-v3.py) と v3 設計検証が資産として残り、着手判断を数字でできる状態でバックログ化できた
- 判断: 現行ユースケース (デスクトップ IME、CF DO 128MB 内、DM250 RAM 1GB で実機動作中) は現状メモリで動いており短期の削減必要なし (ユーザー確認)。看板は「ポータブル」に一本化し、「省メモリ」は Iter 9 完了時に実測をもって掲げ直す
- 残課題: repo public 化の残手順 (ユーザー実行): push → visibility 切替 → interaction limits 設定

### 省メモリの定義確立 (Iter 9 完了条件の再構成) ✓ (2026-08-19)
- やったこと: 「省メモリ」の看板の定義を「物理メモリが少ないデバイスでも動かせること」と確立 (ユーザー決定。言えないなら看板を下ろす)。Iter 9 の主 KPI を cgroup MemoryMax 下での変換完走 (目標 PRD embedded 予算 ~2MB、辞書はストレージに mmap) に再構成し、サイズは副 KPI に。T150 (少メモリ動作検証) と T151 (v3.1 bit 符号化、データ対データの本家超え) を追加。
- 変更したファイル: doc/tasks.md, doc/current.md, doc/done.md
- よかったこと: 勝利条件の議論が「何と比べるか」から「どの環境で動くか」に転換し、PRD のメモリ予算と Iter 9 の目標が一本化された
- 判断: 本家とのサイズ競争はプロキシ指標。mmap ゼロコピーの本質は「辞書全体 ≤ RAM」の要求を「変換時ワーキングセット ≤ RAM」に変えることで、看板の可否は T150 の実測で判定する。WASM は mmap がなく全ロード必須のため、PRD の wasm 予算 6MB は軽量辞書ビルドの領分として Iter 9 から切り離した
- 残課題: 主 KPI ~2MB に実測で届かない場合の看板判断 (T150 で下限の evidence を出す)

### mozc_server 22.7MB の内訳検証 ✓ (2026-08-19)
- やったこと: 比較基準「本家 22.7MB」の中身を一次情報で検証。readelf の ELF セクション実測 (コード .text 2.9MB / .rodata 19.2MB)、埋め込み dataset の magic 確認、TOC (protobuf DataSetMetadata) のパースで46チャンクの内訳を取得し doc/memory.md に記録。
- 変更したファイル: doc/memory.md, doc/done.md
- よかったこと: dataset TOC が protobuf の単純な繰り返しで、アンカーなしでもエントリ単位のパターンマッチで回収できた。チャンク合計 18.9MB が .rodata 19.2MB と整合し検証が閉じた
- 判断: jamo 同等機能のデータは dict 14.65 + conn 2.22 = 16.87MB。v3 見積もり 17.92MB は「バイナリ全体 22.7MB に勝つ」条件は満たすが「データ対データ」では 1.05MB 及ばない — 勝利条件の解釈をユーザーに確認する
- 残課題: 勝利条件の確定 (バイナリ比較 or データ対データ)。後者なら token/表層の bit 単位符号化の追加検討が必要

### v3 サイズの事前検証 (勝算判定) ✓ (2026-08-19)
- やったこと: scripts/estimate-v3.py を追加し、v3 各施策のサイズを mozc 実データで厳密見積もり。素の T141-T149 積み上げは 27.2MB で目標 (.dict ≤ 20MB) に届かないと判明。エントリ境界 BitVec 化・表層の可変長文字符号化・行列例外 varint の3施策を追加して 17.92MB を確認し、tasks.md の設計方針と成果物を実測値で更新。
- 変更したファイル: scripts/estimate-v3.py (新規), doc/memory.md, doc/tasks.md, doc/done.md
- よかったこと: trie ノード数を「ソート済みキーの LCP 法」で構築なしに算出でき、v2 実ノード数と一致して手法を検証できた。設計前のシミュレーションで「作ってから届かない」を回避
- 判断: 表層は trie 化 (4.0MB) と可変長文字符号化 (3.9MB) が同等サイズ → 実装と復元が単純な符号化を採用。読み1バイト符号化のノード削減は 41.9% (1/3 ではない: UTF-8 の先頭バイトは既に共有されていた)。読み文字種 255 は1バイト上限ぎりぎりでエスケープ設計を T148 に明記
- 残課題: T141 設計に3施策を織り込む。estimate-v3.py は v3 実装後に dict info 実測へ役目を譲り削除する

### T140 dict info ✓ (2026-08-19)
- やったこと: jamo-core に dict_stats モジュール新設 (バイナリ v2 のセクション走査)。CLI に `jamo dict info <dict>` サブコマンド追加。テスト4件 (core: セクション合計整合・不正 magic・切断/末尾余剰、CLI: ミニ辞書 build→info 結合)。役目を終えた scripts/dict-breakdown.py を削除し doc/memory.md の再現手順を dict info に更新。
- 変更したファイル: jamo-core/src/dict_stats.rs (新規), jamo-core/src/lib.rs, jamo-cli/src/main.rs, jamo-cli/tests/cli_test.rs, scripts/dict-breakdown.py (削除), doc/memory.md, doc/current.md, doc/done.md
- よかったこと: CSV + matrix.def を temp に生成するミニ辞書パターンで IPAdic 非依存の自己完結テストができた (既存 CLI テストは実辞書がないと skip だった)。実辞書での出力が Python 版と完全一致し移植を検証できた
- 判断: 内訳計算はフォーマット知識なので core (dict_stats)、CLI は表示のみ。ロード実装 (read_binary) と独立のバイト走査にした — v3 追加時に両実装の相互検証として機能する。ipadic.rs (574行、YELLOW 目前) には足さず新モジュール
- 残課題: T141 (v3 フォーマット設計)。v3 対応時に dict_stats も追随が必要

### メモリ実測基盤 + Iter 9 計画 ✓ (2026-08-19)
- やったこと: scripts/bench-memory.py (jamo / Anthy / mozc_server の peak RSS 比較、外部エンジンは work 内で取得・ビルドしシステム無変更) と scripts/dict-breakdown.py (辞書バイナリのセクション内訳) を追加。実測結果と分析を doc/memory.md に記録。省メモリ辞書 (ゼロコピー v3) を Iter 9 (T140-T147) として計画。
- 変更したファイル: scripts/bench-memory.py (新規), scripts/dict-breakdown.py (新規), doc/memory.md (新規), doc/tasks.md, doc/current.md, doc/done.md, README.md
- よかったこと: getrusage(RUSAGE_CHILDREN) は累積 max のため使い捨て子プロセスで測る方式が確実だった。内訳計測で削減余地が定量化できた (terminal の 83% がセンチネル、表層の 41% が生成可能)
- 判断: README の「省メモリ」は実測 (mozc 構成 RSS 152.4MB、Anthy 16.8MB の約9倍) と乖離しており、記述の弱体化ではなく実装での回収を選択 (ユーザー決定)。mozc_server は計測環境でデーモンが即終了し変換込み計測が不能のため、起動直後値 6.2MB を下限として記録し、スクリプトは観測不能時に skip を明示する設計にした
- 残課題: T140-T147 の実装。README の省メモリ記述は Iter 9 完了時に実測値で更新

### プロジェクト方針の整理 (バージョン 0.x・アイデンティティ) ✓ (2026-08-19)
- やったこと: crate バージョンを 0.x に整理 (jamo-core 2.0.0→0.2.0, jamo-cli 1.0.0→0.1.0)。README を「ポータブルなかな漢字変換エンジン」を軸に書き換え、開発状態 (pre-1.0) を明記。
- 変更したファイル: jamo-core/Cargo.toml, jamo-cli/Cargo.toml, Cargo.lock, README.md, doc/current.md, doc/done.md
- よかったこと: git tag も crates.io 公開も皆無だったため、番号の付け直しに移行コストがゼロだった
- 判断: crates.io 未公開で対外依存がなく、実フェーズは pre-1.0 (破壊的変更を恐れない)。semver でも 0.x は「minor で破壊可」でフェーズ表現として正しい。jamo-core は 0.2.0 とし「C API v2」世代との対応を残した。プロジェクトの主体は「ポータブルな変換エンジン」、C API/CLI/HTTP/WASM/IME はインターフェース群 (PRD の CF API 主軸像は初期構想として据え置き)
- 残課題: PRD 全面改訂 (構成図・ロードマップが実態と乖離)

### repo public 化準備 ✓ (2026-08-19)
- やったこと: 秘匿情報スキャン (git 全履歴、クリーン確認)。GitHub 設定 (Projects 無効化、description 設定)。CLAUDE.md のプロジェクト構成を実態に同期 (dicts/ 削除、data/・jamo-ime・scripts 追記)。
- 変更したファイル: CLAUDE.md, doc/current.md, doc/done.md
- よかったこと: LICENSE / Cargo メタデータ / 辞書ライセンス表示 (README・fetch-mozc.sh) が整備済みで追加作業ゼロ。辞書バイナリを gitignore + fetch スクリプト方式にしていたのでライセンス上の配布問題もない
- 判断: 外部 contribution は受け付けない。CONTRIBUTING.md は作らず GitHub の interaction limits (collaborators_only) で制限する。interaction limits は private repo に設定できないため public 化後に適用する。visibility 切替はユーザーが実行する
- 残課題: public 切替 (ユーザー実行)。切替後の interaction limits 設定 (最長6ヶ月で失効、要再設定)

### T132 mozc 辞書ソース対応 ✓ (2026-08-18)
- やったこと:
    - jamo-core/src/mozc.rs 新設: mozc dictionary_oss (dictionary*.txt + id.def + connection_single_column.txt) を IpaDict としてロード。既存のバイナリ形式・エンジンは無変更
    - jamo-cli: 辞書ディレクトリに id.def があれば mozc 形式と自動判別 (dict build / 実行時ロード共通)。デフォルト探索パスに data/mozc(.dict) を追加し IPAdic より優先
    - scripts/fetch-mozc.sh: mozc データ13ファイルのダウンロードスクリプト (README.txt = ライセンス文書込み)
    - accuracy_test: カタカナ語ガード4文を追加 (計56文)、mozc 辞書用の精度テスト (Top-1≥80% / Top-5≥90%) を追加
    - 実測: mozc Top-1 89.3% / Top-5 98.2% (IPAdic 51.8% / 80.4%)。辞書 79.7MB、ロード 0.20s
- 変更したファイル: jamo-core/src/mozc.rs (新規), jamo-core/src/ipadic.rs, jamo-core/src/lib.rs, jamo-cli/src/main.rs, jamo-core/tests/accuracy_test.rs, scripts/fetch-mozc.sh (新規), .gitignore, README.md, doc/tasks.md, doc/current.md, doc/done.md
- よかったこと: id.def の品詞素性が IPAdic 互換形式だったため classify_pos とバイナリ形式をそのまま流用でき、ローダー追加だけで済んだ。スクラッチパッドでの事前実験が部分移植の全滅を早期に示し、実装前に方針を確定できた
- 判断:
    - 当初案の「頻度リストで unigram 再推定」から方針変更 (ask #38/#39 でユーザー承認)。実測により、単語コスト・品詞ID・接続行列は分離不能で、コストのみ移植 12.5%、ブレンド 41.1%、順位のみ移植も文節分割が歪むことを確認。三位一体の丸ごと採用が唯一機能した
    - IPAdic ビルドパスは維持 (naist-jdic 対応 T090 とも整合)
    - 辞書バイナリはコミットしない (gitignore 済み、fetch スクリプトで再現可能)
- 残課題: 組み込み向け軽量ビルド (低頻度語削減で 79.7MB を縮める)。lite-dm2x payload の辞書差し替え。単独短語 (かく→各 等) は学習で対処

### ローマ字 wi/we の現代表記化 ✓ (2026-08-20)
- やったこと: wi/we を うぃ/うぇ に修正し、旧仮名 ゐ/ゑ は wyi/wye へ移した (MS-IME 準拠)。test_wi_we 追加
- 変更したファイル: jamo-core/src/romaji.rs, doc/done.md
- よかったこと: DM250 実機常用が実用バグの検出網になっている (we→ゑ の報告が発端)
- 判断: yi/ye→い/いぇ と同じ現代表記方針に統一。wha/whi/whe/who 系の追加は要望が出るまで見送り
- 残課題: なし

### T160 ローマ字の取りこぼし修正 ✓ (2026-08-23)
- やったこと:
    - `flush()` が「n」以外の未確定子音を捨てていたのを、打った英字のまま返すよう変更 (`kyouk` + Space/Enter で k が消えない)
    - 句読点変換を pending の有無に関わらず適用 (`hon.` → ほん。、`k.` → k。)
    - `xtsu`/`ltsu` → っ、`n'` → ん (アポストロフィ消費) を追加
    - romaji.rs (743 行) をテーブル `romaji/table.rs` とテスト `romaji/tests.rs` に分割
- 変更したファイル: jamo-core/src/romaji.rs, jamo-core/src/romaji/table.rs (新規), jamo-core/src/romaji/tests.rs (新規), doc/done.md, doc/tasks.md, doc/current.md
- よかったこと: probe crate (scratchpad) で feed 列→出力を一括で見る手順が、仕様の粗さをテストケースにそのまま落とせた
- 判断: 未確定子音の扱いは mozc 準拠 (preedit に英字として残す)。捨てる方が「きれい」に見えるが、入力が黙って失われる方が害が大きい
- 残課題: なし
