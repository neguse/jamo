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
