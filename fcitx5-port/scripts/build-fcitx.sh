#!/bin/bash
# fcitx5-windows 完整构建（MSYS2 clang64 环境，schtasks 完整令牌下运行）
export MSYSTEM=CLANG64
export MSYS2_PATH_TYPE=inherit
export LC_ALL=C
export HOME=/home/root
LOG=/g/msys64/tmp/build-fcitx.log
exec > $LOG 2>&1
echo "=== build fcitx5 start $(date) ==="
cd /g/WowType/fcitx5-windows || { echo "NO SOURCE"; exit 1; }

# 1. fcitx5 核心 + 主程序
echo "--- configure fcitx5 core ---"
cmake -B build/x86_64 -G Ninja -DCMAKE_BUILD_TYPE=Release -DARCH=AMD64 2>&1 | tail -3
echo "--- build ---"
cmake --build build/x86_64 2>&1 | tail -3
echo "--- install ---"
cmake --install build/x86_64 --prefix /g/WowType/fcitx5-inst 2>&1 | tail -3

# 2. win32 TSF DLL
echo "--- configure win32 ---"
cd win32 || exit 1
cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release 2>&1 | tail -3
echo "--- build win32 ---"
cmake --build build 2>&1 | tail -3

echo "=== build done $(date) ==="
ls -la build/dll/*.dll 2>/dev/null || echo "no tsf dll"
ls /g/WowType/fcitx5-inst/bin 2>/dev/null | head -10
