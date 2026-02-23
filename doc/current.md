# 現在の状態

## イテレーション進捗

| Iter | 状態 | テーマ |
|------|------|--------|
| 0 | 完了 | 足場 |
| 1 | ほぼ完了 | 針金 (最薄E2E) |
| 2 | ほぼ完了 | 実データ |
| 3 | 完了 | 運用 |
| 4 | 進行中 | 品質→1.0 |

## Iter 3 完了タスク

| タスク | 備考 |
|--------|------|
| T041/T042 CF Worker + DO | WASM 144KB, wrangler dry-run OK |
| T043 WebSocket | /ws, Hibernation API |
| T044 学習テーブル | SQLite user_history, LRU 10K |
| T045 ユーザー辞書 API | /dict/add,remove,list,import |
| T046 予測変換 | prefix_search + /predict |
| T047 ドメイン辞書 | ?domain=X |
| T048 C API | cbindgen, jamo.h, 497KB .so |

## Iter 4 進捗

| タスク | 状態 | 備考 |
|--------|------|------|
| T050 ベンチマーク | 完了 | convert 10-123µs, dict_load 64ms |
| T051 精度評価 | 完了 | 10カテゴリ別, 文一致 48.1%, 文字精度 69.1% |
| T052 NEologd | 未着手 | |
| T053 API仕様 | 完了 | OpenAPI 3.0.3, doc/api.yaml |
| T054 辞書フォーマット | 完了 | doc/dict-format.md, v2 |
| T055 ドキュメント | 未着手 | |
| T056 1.0 リリース | 未着手 | |

## パフォーマンス

| 項目 | 値 |
|------|-----|
| 変換 (短文) | 10µs |
| 変換 (中文) | 87µs |
| 変換 (長文) | 99µs |
| 辞書ロード | 64ms |
| prefix_search | 1-2µs |
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

全66テスト通過 (jamo-core: 58 unit + 2 accuracy, jamo-cli: 4 CLI + 2 HTTP)

## 残タスク

- Iter 2 残: T022 bigram LM (精度改善の鍵)
- Iter 4 残: T052 NEologd, T055 ドキュメント, T056 1.0リリース
