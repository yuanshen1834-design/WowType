#!/bin/bash
# 生成精简词库：全部单字 + 高频词 top N（减小引擎内存 433MB → ~50MB）
set -e
SRC=/mnt/g/WowType/app/data/lexicon.tsv
OUT=/mnt/g/WowType/app/data/lexicon_small.tsv
TOP_N=${1:-50000}

echo "=== 源词库 ==="
wc -l "$SRC"

echo "=== 提取单字（所有 1 字词条，含生僻字）==="
awk -F'\t' '!/^#/ && length($2)==1' "$SRC" > /tmp/singles.tsv
wc -l /tmp/singles.tsv

echo "=== 提取高频词 top $TOP_N ==="
awk -F'\t' '!/^#/ && length($2)>1' "$SRC" | sort -t$'\t' -k3,3nr | head -n "$TOP_N" > /tmp/top.tsv
wc -l /tmp/top.tsv

echo "=== 合并（注释头 + 单字 + 高频词）==="
head -2 "$SRC" > "$OUT"
cat /tmp/singles.tsv >> "$OUT"
cat /tmp/top.tsv >> "$OUT"
wc -l "$OUT"
du -h "$OUT"
