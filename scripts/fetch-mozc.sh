#!/bin/sh
# mozc dictionary_oss (語彙・コスト・品詞ID・接続行列) をダウンロードする。
# ライセンス: IPAdic license + BSD-3-Clause (mozc) + Okinawa dictionary license。
# 詳細は取得される README.txt と LICENSE を参照。
# env MOZC_REF: 取る mozc の commit か branch (既定 master)。辞書を配る build は commit を固定する。
set -e
dir="${1:-data/mozc}"
ref="${MOZC_REF:-master}"
root="https://raw.githubusercontent.com/google/mozc/$ref"
base="$root/src/data/dictionary_oss"
mkdir -p "$dir"
for f in dictionary00.txt dictionary01.txt dictionary02.txt dictionary03.txt \
         dictionary04.txt dictionary05.txt dictionary06.txt dictionary07.txt \
         dictionary08.txt dictionary09.txt id.def connection_single_column.txt \
         README.txt; do
    echo "fetch $f"
    curl -fsSL "$base/$f" -o "$dir/$f"
done
# 文節の先頭/末尾に来る品詞へのペナルティ (本家 converter/segmenter の boundary データ)
echo "fetch boundary.def"
curl -fsSL "$root/src/data/rules/boundary.def" -o "$dir/boundary.def"
# mozc の BSD-3-Clause の表示。辞書バイナリを配るときに README.txt と一緒に同梱する
echo "fetch LICENSE"
curl -fsSL "$root/LICENSE" -o "$dir/LICENSE"
echo "done: $dir ($ref)"
echo "build: cargo run -p jamo-cli --release -- dict build --dict $dir -o data/mozc.dict"
