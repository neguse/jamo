#!/bin/sh
# mecab-ipadic (CSV辞書ソース + 接続行列) をダウンロードする。
# ライセンス: IPAdic license。詳細は mecab リポジトリの mecab-ipadic/COPYING を参照。
# ソースは EUC-JP のため、jamo が要求する UTF-8 へ変換して配置する。
set -e
dir="${1:-data/ipadic}"
url="https://codeload.github.com/taku910/mecab/tar.gz/refs/heads/master"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "fetch mecab-ipadic source"
curl -fsSL "$url" -o "$tmp/mecab.tar.gz"
tar -xzf "$tmp/mecab.tar.gz" -C "$tmp" mecab-master/mecab-ipadic

mkdir -p "$dir"
for f in "$tmp"/mecab-master/mecab-ipadic/*.csv "$tmp"/mecab-master/mecab-ipadic/*.def; do
    name=$(basename "$f")
    echo "convert $name"
    iconv -f EUC-JP -t UTF-8 "$f" -o "$dir/$name"
done

echo "done: $dir"
echo "build: cargo run -p jamo-cli --release -- dict build -o data/ipadic.dict"
