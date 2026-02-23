# 現在の状態

## イテレーション進捗

| Iter | 状態 | テーマ |
|------|------|--------|
| 0 | 完了 | 足場 |
| 1 | ほぼ完了 | 針金 (最薄E2E) |
| 2 | - | 実データ |
| 3 | - | 運用 |
| 4 | - | 品質→1.0 |

## Iter 1 進捗

| タスク | 状態 | 備考 |
|--------|------|------|
| T010 ハードコード辞書 | 完了 | HashMap, 10語, 7テスト |
| T011 ラティス + Viterbi | 完了 | BOS/EOS付き, 16テスト |
| T012 jamo convert CLI | 完了 | E2E結合テスト3件 |
| T013 jamo serve HTTP | 完了 | axum, /convert, /health, HTTPテスト2件 |
| T014 WASM ビルド | 未着手 | rustup/wasm-pack 未インストール |

## 確認済みの動作

```
$ cargo run -p jamo-cli -- convert "きょうは"
今日は

$ curl "localhost:8080/convert?q=きょうは"
{"result":[{"yomi":"きょう","candidates":["今日"]},{"yomi":"は","candidates":["は"]}]}
```

## 既知の問題

- 接続コストなしのため「てんき」→「天+木」になる (Iter 2 で対応)
- 辞書はリクエストごとに生成 (共有化が必要)
- N-best 未対応

## テスト

全21テスト通過 (jamo-core: 16, jamo-cli: 3 CLI + 2 HTTP)

## 次のアクション

Iter 2 開始: T020 IPAdic パース
(T014 WASM は wasm-pack インストール後に対応)
