# 現在の状態

## バージョン

**v1.0.0** (2026-02-24)

## イテレーション進捗

| Iter | 状態 | テーマ |
|------|------|--------|
| 0 | 完了 | 足場 |
| 1 | 完了 | 針金 (最薄E2E) |
| 2 | ほぼ完了 | 実データ |
| 3 | 完了 | 運用 |
| 4 | 完了 | 品質→1.0 |

## Iter 4 完了タスク

| タスク | 備考 |
|--------|------|
| T050 ベンチマーク | criterion, convert 10-123us, dict_load 64ms |
| T051 精度評価 | 10カテゴリ別, 文一致 48.1%, 文字精度 69.1% |
| T052 NEologd | load_merge + dict merge CLI |
| T053 API仕様 | OpenAPI 3.0.3, doc/api.yaml |
| T054 辞書フォーマット | doc/dict-format.md, v2 |
| T055 ドキュメント | README.md |
| T056 1.0 リリース | LICENSE, Cargo.toml メタデータ, v1.0.0 タグ |

## パフォーマンス

| 項目 | 値 |
|------|-----|
| 変換 (短文) | 10us |
| 変換 (中文) | 87us |
| 変換 (長文) | 99us |
| 辞書ロード | 64ms |
| prefix_search | 1-2us |
| WASM バイナリ | 144KB |
| 辞書バイナリ | 19.2MB |
| 共有ライブラリ | 497KB |

## 精度 (IPAdic + Viterbi, bigram LMなし)

| カテゴリ | 正解率 |
|---------|--------|
| VerbConjugation | 100% |
| Place | 100% |
| Time | 100% |
| NounPhrase | 60% |
| Complex | 50% |
| Polite | 50% |
| ShortVerb | 43% |
| Daily | 40% |
| ShortNoun | 30% |
| Adjective | 0% |
| **全体** | **48.1%** (文字精度 69.1%) |

## テスト

全68テスト通過 (jamo-core: 60 unit + 2 accuracy, jamo-cli: 4 CLI + 2 HTTP)

## 1.0 後の課題

- T022 bigram LM (精度改善の鍵, 現在48.1%→目標70%+)
- T060 trigram LM
- T061 NN リランキング
- T062 長文脈変換
- CONTRIBUTING.md
- NEologd CSV 自動ダウンロードスクリプト
