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
| 5 | 進行中 | Windows IME |

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

### 残タスク

- Windows 実機テスト (regsvr32 → メモ帳)
- 辞書同梱インストーラー

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

全111テスト通過:
- jamo-core: 95 (60 unit + 35 romaji + 2 accuracy)
- jamo-cli: 6 (4 CLI + 2 HTTP)
- jamo-ime: 16 (composition 状態マシン)

## プロジェクト構成

```
jamo/
├── jamo-core/          # 変換エンジン (Rust lib)
│   └── src/romaji.rs   # [NEW] ローマ字→ひらがな変換 FSM
├── jamo-cli/           # CLI・サーバー (Rust bin)
├── jamo-ime/           # [NEW] Windows IME (TSF COM DLL)
│   └── src/
│       ├── lib.rs              # DllMain + 4 COM exports
│       ├── globals.rs          # GUID, 定数, グローバル状態
│       ├── composition.rs      # 状態マシン (Platform非依存)
│       ├── class_factory.rs    # IClassFactory
│       ├── text_service.rs     # ITfTextInputProcessorEx
│       ├── edit_session.rs     # EditSession パターン
│       ├── display_attribute.rs # 3種下線
│       ├── engine.rs           # jamo-core ラッパー
│       ├── candidate_ui.rs     # 候補ウィンドウ
│       └── registry.rs         # COM/TSF 登録
├── jamo-worker/        # CF Worker/DO (TypeScript)
├── doc/                # ドキュメント
├── dicts/              # 辞書
└── .github/workflows/  # CI (Linux + Windows)
```

## 1.0 後の課題

- T022 bigram LM (精度改善の鍵, 現在48.1%→目標70%+)
- T060 trigram LM
- T061 NN リランキング
- T062 長文脈変換
- CONTRIBUTING.md
- NEologd CSV 自動ダウンロードスクリプト
