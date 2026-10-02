# 本家 mozc との比較ベンチ

ゴール「mozc より省メモリ・高速で、変換精度は同等以上」を同一コーパスで検証する。
再計測:

```bash
cargo build --release -p jamo-cli
scripts/fetch-mozc.sh && cargo run -p jamo-cli --release -- dict build --dict data/mozc -o data/mozc.dict
# root のコンテナでは mozc_server が起動を拒否するので一般ユーザーを指定する
sudo useradd -m bench
sudo python3 scripts/bench-engines.py --user bench --drop-caches --repeat 20
```

## 結果 (2026-10-02, Linux x86_64 6.18, コーパス 56 文, repeat 20, cold start)

| 指標 | jamo (mozc 辞書) | mozc_server (本家 3.34.6239) |
|------|------------------|--------------------|
| Top-1 | 54/56 (96.4%) | 54/56 (96.4%) |
| Top-5 (候補選択で到達可能) | 55/56 (98.2%) | 55/56 (98.2%) |
| 変換レイテンシ 平均 | **57 us** | 1246 us |
| 変換レイテンシ p50 / p95 | **33 / 128 us** | 930 / 2561 us |
| 起動 (辞書ロード) | **2.4 ms** | 116.6 ms |
| 辞書 / バイナリ | **19.3 MB** | 22.7 MB |
| peak RSS (VmHWM、56 文 × 21 回変換後) | **8.5 MB** | 25.2 MB |
| 起動直後 RSS (mozc) | - | 17.3 MB |

両エンジンが外した 2 文 (同じ外し方):

| 入力 | 期待 | jamo | mozc |
|------|------|------|------|
| あめがふっているからかさをもつ | 雨が降っているから傘を持つ | 雨が振っている傘を持つ | 雨が振っている傘を持つ |
| さむいのであたたかいものをたべる | 寒いので暖かいものを食べる | 寒いので温かいものを食べる | 寒いので温かいものを食べる |

「からかさ」は辞書に「傘」の読みとしてあり、「あたたかいもの」は文脈依存。どちらも
本家と同じ結果なので、辞書・接続行列・境界ペナルティが同じ以上は妥当な挙動とみなす。

## 計測方法 (scripts/bench-engines.py)

- コーパスは `jamo-core/tests/corpus.tsv` (accuracy_test.rs と共用)。
- jamo: `jamo-cli bench` でプロセス内変換。レイテンシは各文を 1 回ウォームアップ後
  repeat 回変換した平均。メモリは `/proc/self/status` の VmHWM。
- mozc: `mozc_server` (Arch Linux の fcitx5-mozc パッケージを展開) を起動し、本家
  クライアントと同じ IPC で変換する。abstract unix socket
  `\0tmp/.mozc.<key>.session` (key は `~/.config/mozc/.session.ipc` の protobuf) に
  `commands.Input` を生で送って half-close し、`commands.Output` を受ける。
  かなは `KeyEvent.key_string` + `AS_IS` で入力し、Space で変換、`preedit.segment`
  の連結を Top-1、`all_candidate_words` (Right で文節を移しながら) の上位 5 件を
  Top-5 の候補にする。各文は REVERT で捨てる (学習させない)。
  レイテンシは「かな入力 + Space」の 2 往復 (IPC 込み。fcitx5-mozc も同じ経路で
  サーバーと話す)。メモリは mozc_server の VmHWM。protobuf は必要フィールドだけ
  手で符号化・復号するので依存ライブラリ不要。
- `--drop-caches` でページキャッシュを捨てて cold start で測る。Linux 6.x のページ
  キャッシュは large folio で、キャッシュ済みファイルを mmap すると 1 バイト触れた
  だけで 2MB が RSS に数えられる (jamo の辞書は 19.3 MB 全体に薄く触れるので
  warm だと 22 MB と出る)。捨ててから測ると実際に触れた 4KB ページだけになる。
  両エンジンに同じ条件で適用される。
- mozc_server は root では起動を拒否する (geteuid()==0 で abort)。IPC は同じ uid の
  クライアントしか受け付けないので、root のときは `--user` で指定したユーザーで
  スクリプト全体を再実行する。

## 精度を揃えた変更

初回計測では jamo の Top-1 は 50/56 で、本家に「いく→行く」「くる→来る」「かく→書く」
「かわ→川」の単文節 4 件で負けていた。いずれも非自立動詞 (〜ていく) や接頭詞 (各)、
接尾辞 (側) のコストの低い語が単独で勝っていたもの。本家は converter/segmenter の
boundary データ (src/data/rules/boundary.def) で、変換キーの先頭に来る語に品詞ごとの
PREFIX ペナルティ、末尾に来る語に SUFFIX ペナルティを足している (動詞,非自立 の
PREFIX は 4000、接頭詞 の SUFFIX は 4000)。同じ規則を `jamo-core/src/boundary.rs`
で読み、辞書ビルド時に品詞 ID ごとの表 (v3 の boundary セクション) にして、ラティス
構築時に先頭/末尾ノードの単語コストへ足す。これで 4 件が直り 54/56 で本家と同点。

## 注意

- 56 文は回帰検知用の小さなコーパスで、統計的に本家と差があるとは言えない。
  同点は「同じ辞書・行列・境界規則で同じ答えになる」ことの確認。
- 本家は IPC サーバー構成なのでレイテンシに往復 (約 0.3 ms × 2) が含まれる。それを
  差し引いても 10 倍以上の差がある。
- 本家のメモリは変換・学習・予測・rewriter 群を含む多機能デーモンの値。jamo は
  変換エンジン単体。
