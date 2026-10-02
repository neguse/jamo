# 現在の状態

## バージョン

**0.2.0** (2026-08-19) — crates.io 未公開につき pre-1.0 の 0.x に整理。C API は第2世代 (セッション型)。辞書フォーマットは v3 (ゼロコピー、2026-10-01、v2 は読めないので `jamo dict build` で再構築)。

## イテレーション進捗

| Iter | 状態 | テーマ |
|------|------|--------|
| 0 | 完了 | 足場 |
| 1 | 完了 | 針金 (最薄E2E) |
| 2 | ほぼ完了 | 実データ |
| 3 | 完了 | 運用 |
| 4 | 完了 | 品質→1.0 |
| 5 | 進行中 | Windows IME |
| 8 | 進行中 | 変換精度・C API v2 (T130-T134 完了) |
| 9 | ほぼ完了 | 省メモリ辞書 (ゼロコピー v3): T140-T149 完了。.dict 83.5→19.3MB、peak RSS 152.6→17.9MB (ext4) / 9.5MB (tmpfs)。残: T150 cgroup 検証、T151 bit 符号化 |
| 10 | 完了 | 入力体験 (キー一打の手触り): T160-T165 完了。lite-dm2x の host ハーネス (`just test-ime`) と DM250 実機 (2026-08-25、keyin 注入 + screenshot) で検証済み |
| 11 | 完了 | Linux IME (fcitx5): T170 アドオン + E2E (CI は Arch コンテナ)、T171 Omarchy 実機導入 |
| 12 | 完了 | 本家 mozc との比較: T180 bench-engines.py (IPC で mozc_server を駆動)、T181 文節境界ペナルティ (boundary.def) |

## ゴール (2026-10-01): mozc より省メモリ・高速で、精度は同等以上

同一コーパス (56 文) で本家 mozc_server と直接比較した (scripts/bench-engines.py、doc/bench.md、2026-10-02、cold start):

| 軸 | jamo v3 (mozc 辞書) | 本家 mozc_server 3.34 | 状態 |
|----|--------------------|-----------------|------|
| 辞書バイナリ | **19.3 MB** (.dict) | 22.7 MB (バイナリ埋め込み、データ部 16.87 MB) | 勝ち (バイナリ対バイナリ)。データ対データは 2.4 MB 及ばない (T151) |
| peak RSS (56 文 × 21 回変換後) | **8.5 MB** | 25.2 MB (起動直後 17.3 MB) | 勝ち |
| 変換レイテンシ 平均 / p50 / p95 | **57 / 33 / 128 us** | 1246 / 930 / 2561 us (IPC 往復込み) | 勝ち (往復 ~0.6 ms を差し引いても 10 倍以上) |
| 起動 | **2.4 ms** | 116.6 ms | 勝ち |
| 精度 Top-1 / Top-5 | 54/56 / 55/56 | 54/56 / 55/56 | 同点。初回は 50/56 で単文節 4 件負けていたが、本家の boundary.def (文節境界ペナルティ) を移植して同点 |

残る 2 文 (からかさ→傘、あたたかい→温かい) は本家も同じ外し方。

次のアクション: (1) T151 の bit 符号化でデータ部 16.87 MB を下回る、(2) T150 の cgroup
検証、(3) もっと大きいコーパス (本家との差が統計的に出る規模) での精度比較。

## Iter 11: Linux IME (fcitx5)

`jamo-fcitx5/` (C++ アドオン、jamo-core を staticlib でリンク)。testfrontend による E2E
(`ctest --test-dir jamo-fcitx5/build`)。この Linux 機 (Omarchy, Hyprland 0.56) に
`~/.local` で導入済み (README の手順、DefaultIM=jamo、元設定は `~/.config/fcitx5/profile.bak-mozc`)。

候補一覧は mozc / MS-IME 式 (最初の Space は変換のみ、次の Space / ↑↓ で表示)。
Hyprland / sway は text-input のカーソル矩形の更新で popup を置き直さないため、preedit と
同時に出すと古い位置に出る (tasks.md Iter 11 の設計決定)。

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

| 項目 | v3 (2026-10-01) | v2 (同環境で再計測) |
|------|-----|-----|
| 変換 (短文) | 12.8us | 14.0us |
| 変換 (中文) | 95us | 114us |
| 変換 (長文) | 107us | 122us |
| 変換 (複雑) | 131us | 157us |
| 辞書ロード (IPAdic) | 93us (mmap) | 87ms |
| prefix_search | 1.9-5.9us | 1.6-3.0us |
| WASM バイナリ | 144KB | |
| 辞書バイナリ (IPAdic) | 8.4MB | 19.2MB |
| 辞書バイナリ (mozc) | 19.3MB (ロード 0.1ms) | 79.7MB (ロード 0.20s) |
| peak RSS (mozc 辞書、変換 1 回) | 17.9MB ext4 / 9.5MB tmpfs | 152.6MB |

criterion (IPAdic、`cargo bench -p jamo-core`)。v3 の変換が速いのは Viterbi の前ノード
集約 (right_id ごと) と (開始位置, left_id) ごとの最良メモ化で接続コスト参照が激減した
ため (接続コスト 1 回は v2 の 1.7ns → v3 4.7ns と遅いが、回数が 1/5 以下)。
実行時メモリ (peak RSS) の実測と他エンジン比較は doc/memory.md を参照。

## 精度

### mozc 辞書 (推奨、56文コーパス実測)

| 指標 | 値 |
|------|-----|
| Top-1 | **96.4%** (54/56、本家 mozc_server と同点) |
| Top-5 (候補選択で到達可能) | **98.2%** |

`scripts/fetch-mozc.sh && jamo dict build --dict data/mozc -o data/mozc.dict` で構築。
辞書バイナリ 79.7MB、ロード 0.20s。残る失敗は文脈依存 (温かい/暖かい 等) と
単独短語の同音語で、commit 学習で恒久修正される類。

### IPAdic (参考、bigram LMなし)

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
対処として mozc 辞書ソース (T132) を推奨とした。

## 組み込み C API (v2: セッション型)

単一プロセス組み込み (DM250 / lite-dm2x 等) 向けに capi.rs で公開。
v2 で JSON 返しのステートレス API (jamo_convert/jamo_resize/jamo_predict/
jamo_romaji_*) を廃止し、セッションハンドル + アクセサに刷新:
`jamo_session_*` (new/free/feed/input/backspace/preedit/focused_range/
convert/clause_count/clause_yomi/candidate_count/candidate/selected/select/
focus/set_focus/resize/revert/commit/reset)。
状態機械は jamo-core/src/session.rs (プラットフォーム非依存、キーイベント
処理は含まない)。返却文字列は次の呼び出しまで有効で解放不要。
Iter 10 で追加: preedit カーソル (`cursor/move_cursor/set_cursor/
delete_forward`)、一時英字モード (`is_alpha/set_alpha`、大文字入力で自動 on)、
文字種変換 (`transform` + `JAMO_TRANSFORM_*`: ひらがな/カタカナ/半角英字/
全角英字)、部分確定 (`commit_prefix`)。学習は直前文節の表層を文脈に持つ
(TSV 3 列、旧 2 列も読める)。
commit 学習 (learning.rs の読み→表層 LRU、エンジン単位で全セッション共有)
を内蔵し、`jamo_engine_save_user_data` / `jamo_engine_load_user_data` で
ホスト提供パス (DM250 は SD) に永続化できる。
ヘッダは cbindgen 生成 (include/jamo.h、`~/.cargo/bin/cbindgen --config
jamo-core/cbindgen.toml -o jamo-core/include/jamo.h jamo-core`)。lite-dm2x 側
(api_jamo.c / ime.lua) は rev 7a8ecad に追随済み。Windows の composition.rs は
独自状態機械のままで Session の新機能 (英字モード・カーソル・transform) は未対応。

## テスト

全241テスト通過 (2026-10-02 実測、cargo test。IPAdic / mozc データあり)

## プロジェクト構成

```
jamo/
├── jamo-core/          # 変換エンジン (Rust lib)
│   └── src/
│       ├── bindict/    # v3 ゼロコピー辞書 (format / build / charcode)
│       ├── bitvec.rs   # rank/select ビット列 (BitVec = ビルド、BitView = 借用)
│       ├── ipadic.rs   # 辞書ソース (IPAdic CSV / mozc TSV) のパース → v3 ビルド
│       ├── ipc.rs      # IPC プロトコル型 (ipc feature)
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

## repo 公開

準備完了 (2026-08-19)。秘匿情報は git 全履歴スキャンでクリーン確認済み。
外部 contribution は受け付けない方針 (CONTRIBUTING.md は作らず GitHub 設定で制限)。
残手順 (ユーザー実行):

1. `gh repo edit neguse/jamo --visibility public --accept-visibility-change-consequences`
2. `gh api repos/neguse/jamo/interaction-limits -X PUT -f limit=collaborators_only -f expiry=six_months`
   — 外部からの PR/issue/comment をブロック。最長6ヶ月で失効するため要再設定

## 1.0 後の課題

- T022 bigram LM (mozc 辞書採用で優先度低下。残る文脈依存誤りの解消用。
  mozc OSS の collocation.txt はスタブで連語データは無い。当面は文脈つき学習 T164 で対処)
- 予測変換 (入力途中の候補提示)。Iter 10 では射程外とした
- Windows composition.rs を Session に乗せ換え (英字モード・カーソル・transform を Windows でも)
- 日付・時刻候補 (きょう → 2026/08/23)、全角数字入力の数字ノード化
- 組み込み向け軽量辞書ビルド (mozc 79.7MB の低頻度語削減)
- T060 trigram LM
- T061 NN リランキング
- T062 長文脈変換
- NEologd CSV 自動ダウンロードスクリプト
