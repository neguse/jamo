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
