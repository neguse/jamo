# 現在の状態

## イテレーション進捗

| Iter | 状態 | テーマ |
|------|------|--------|
| 0 | 完了 | 足場 |
| 1 | ほぼ完了 | 針金 (最薄E2E) |
| 2 | ほぼ完了 | 実データ |
| 3 | 完了 | 運用 |
| 4 | - | 品質→1.0 |

## Iter 3 進捗

| タスク | 状態 | 備考 |
|--------|------|------|
| T041 CF Worker | 完了 | ルーター + Bearer認証 → DO ルーティング |
| T042 Durable Object | 完了 | WASM統合 + R2辞書ロード, wrangler dry-run OK |
| T043 WebSocket | 完了 | /ws エンドポイント, JSON メッセージ, Hibernation API |
| T044 学習テーブル | 完了 | SQLite user_history, commit→reorder, LRU 10K |
| T045 ユーザー辞書 API | 完了 | /dict/add,remove,list,import + WS対応 |
| T046 予測変換 | 完了 | prefix_search + /predict + WS predict |
| T047 ドメイン辞書 | 完了 | ?domain=X, R2 {domain}.dict, エンジンキャッシュ |
| T048 C API | 完了 | cbindgen, jamo.h, libjamo_core.so 497KB |

## 確認済みの動作 (IPAdic + LOUDS + バイナリ)

```
わたしはがくせいです → 私は学生です
とうきょうにいきたい → 東京に行きたい
にほんごをべんきょうする → 日本語を勉強する
```

## WASM + CF Worker

- wasm-pack ビルド: 144KB WASM バイナリ
- wrangler dry-run: 157KB バンドル (WASM含む)
- DO: R2 から辞書ロード (lazy init)、/convert + /resize + /ws エンドポイント
- デプロイ未実施 (R2バケット作成 + 辞書アップロードが必要)

## 既知の問題

- 「きょう」→「教」（「今日」が期待値）— 接続コストの精度限界
- バイナリ辞書 19.2MB (v2, pos_type追加)

## テスト

全66テスト通過 (jamo-core: 58 unit + 2 accuracy, jamo-cli: 4 CLI + 2 HTTP)

## 次のアクション

Iter 2 残: T022 bigram LM (精度改善)
Iter 3 完了。Iter 4 開始可能: T050 ベンチマーク / T051 精度評価
Iter 2 残: T022 bigram LM (精度改善)
