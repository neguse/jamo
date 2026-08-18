#!/bin/sh
# mozc dictionary_oss (語彙・コスト・品詞ID・接続行列) をダウンロードする。
# ライセンス: IPAdic license + BSD-3-Clause (mozc) + Okinawa dictionary license。
# 詳細は取得される README.txt を参照。
set -e
dir="${1:-data/mozc}"
base="https://raw.githubusercontent.com/google/mozc/master/src/data/dictionary_oss"
mkdir -p "$dir"
for f in dictionary00.txt dictionary01.txt dictionary02.txt dictionary03.txt \
         dictionary04.txt dictionary05.txt dictionary06.txt dictionary07.txt \
         dictionary08.txt dictionary09.txt id.def connection_single_column.txt \
         README.txt; do
    echo "fetch $f"
    curl -fsSL "$base/$f" -o "$dir/$f"
done
echo "done: $dir"
echo "build: cargo run -p jamo-cli --release -- dict build --dict $dir -o data/mozc.dict"
