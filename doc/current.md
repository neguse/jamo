# 現在の状態

## バージョン

**v2.0.0** (2026-08-18) — C API をセッション型に刷新 (破壊的変更)。エンジン・辞書は v1 と互換。

## イテレーション進捗

| Iter | 状態 | テーマ |
|------|------|--------|
| 0 | 完了 | 足場 |
| 1 | 完了 | 針金 (最薄E2E) |
| 2 | ほぼ完了 | 実データ |
| 3 | 完了 | 運用 |
| 4 | 完了 | 品質→1.0 |
| 5 | 進行中 | Windows IME |
| 8 | 進行中 | 変換精度・C API v2 (T130, T131, T133, T134 完了) |

## Iter 5: Windows IME

### 完了タスク

| タスク | 備考 |
|--------|------|
| T070 ローマ字エンジン | romaji.rs, 35テスト, ~200パターン |
| T071 jamo-ime crate 骨格 | cdylib, DllMain + 4 COM exports |
| T072 COM/TSF 登録 | registry.rs (CLSID, Profile, Category) |
| T073 IClassFactory | class_factory.rs |
| T074 TextInputProcessor | text_service.rs (Activate/Deactivate) |
| T075 KeyEventSink | OnTestKeyDown/OnKeyDown |
| T076 EditSession | edit_session.rs |
| T077 Composition 状態マシン | composition.rs, 16テスト, Platform非依存 |
| T078 ひらがな Composition | ローマ字→ひらがな + TSF統合 |
| T079 DisplayAttribute | 3種下線 (Input/Converted/Focused) |
| T080 かな漢字変換統合 | engine.rs (辞書ロード + convert/resize/predict) |
| T081-T083 候補選択/文節伸縮/候補UI | Space/↓/←/→, Shift+←/→, OS標準UI |
| T084 GHA Windows CI | windows-latest ビルド + テスト |
| T085 IME マルチプロセス化 | IPC プロトコル + Named Pipe + DLL プロキシ化 |
| T086 デプロイスクリプト | scripts/deploy-ime.ps1 |

### 残タスク

- Windows 実機テスト (`jamo serve --ime` + regsvr32 → メモ帳)
- 辞書同梱インストーラー
- IPC タイムアウト設定
- サーバー自動起動 (DLL からサーバープロセスを spawn)

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

Top-5 到達率 (各文節の上位5候補から選んで正解に到達できる率): **78.8%**

主因は分析済み (Iter 8 参照): カタカナ恒等表層のコストが漢字表層より低い
(ホン 3692 vs 本 5947)、IPAdic 単語コストが変換用途向けでない (核 3422 vs 書く 7325)。
対処は doc/tasks.md Iter 8 の T131 (フォールバック候補)・T132 (単語コスト再推定)。

## 組み込み C API (v2: セッション型)

単一プロセス組み込み (DM250 / lite-dm2x 等) 向けに capi.rs で公開。
v2 で JSON 返しのステートレス API (jamo_convert/jamo_resize/jamo_predict/
jamo_romaji_*) を廃止し、セッションハンドル + アクセサに刷新:
`jamo_session_*` (new/free/feed/input/backspace/preedit/focused_range/
convert/clause_count/clause_yomi/candidate_count/candidate/selected/select/
focus/set_focus/resize/revert/commit/reset)。
状態機械は jamo-core/src/session.rs (プラットフォーム非依存、キーイベント
処理は含まない)。返却文字列は次の呼び出しまで有効で解放不要。
commit 学習 (learning.rs の読み→表層 LRU、エンジン単位で全セッション共有)
を内蔵し、`jamo_engine_save_user_data` / `jamo_engine_load_user_data` で
ホスト提供パス (DM250 は SD) に永続化できる。
ヘッダは cbindgen 生成 (include/jamo.h)。lite-dm2x 側の追随
(api_jamo.c / ime.lua / abi_smoke.c) は未着手。

## テスト

全178テスト通過 (2026-08-18 実測、cargo test)

## プロジェクト構成

```
jamo/
├── jamo-core/          # 変換エンジン (Rust lib)
│   └── src/
│       ├── ipc.rs      # [NEW] IPC プロトコル型 (ipc feature)
│       └── romaji.rs   # ローマ字→ひらがな変換 FSM
├── jamo-cli/           # CLI・サーバー (Rust bin)
│   └── src/
│       ├── ime_server.rs # [NEW] IME IPC サーバー (Named Pipe / Unix Socket)
│       ├── main.rs       # convert/serve/dict + serve --ime
│       └── serve.rs      # HTTP API サーバー
├── jamo-ime/           # Windows IME (TSF COM DLL → IPC プロキシ)
│   └── src/
│       ├── engine.rs   # [CHANGED] IPC クライアント (辞書ロード削除)
│       └── ...         # TSF COM 実装
├── scripts/            # [NEW] 開発スクリプト
│   └── deploy-ime.ps1  # DLL デプロイ (リネーム方式)
├── jamo-worker/        # CF Worker/DO (TypeScript)
├── doc/                # ドキュメント
└── .github/workflows/  # CI (Linux + Windows)
```

## 1.0 後の課題

- T022 bigram LM (精度改善の鍵, 現在48.1%→目標70%+)
- T060 trigram LM
- T061 NN リランキング
- T062 長文脈変換
- CONTRIBUTING.md
- NEologd CSV 自動ダウンロードスクリプト
