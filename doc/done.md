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
