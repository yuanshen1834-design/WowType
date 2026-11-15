#!/bin/bash
# 测试 WSL 里可用的下载源
echo '=== 直连 ==='
for u in \
  'https://rsproxy.cn/rustup/dist/x86_64-unknown-linux-gnu/rustup-init' \
  'https://mirrors.tuna.tsinghua.edu.cn/rustup/dist/x86_64-unknown-linux-gnu/rustup-init' \
  'https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init'
do
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 10 -L "$u" 2>/dev/null)
  echo "$u => $code"
done
echo '=== 走代理 ==='
for u in \
  'https://rsproxy.cn/rustup/dist/x86_64-unknown-linux-gnu/rustup-init' \
  'https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init'
do
  code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 15 -L -x http://127.0.0.1:7890 "$u" 2>/dev/null)
  echo "proxy $u => $code"
done
