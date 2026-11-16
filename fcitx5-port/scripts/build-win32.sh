#!/bin/bash
# win32 TSF DLL 构建（链接 fcitx5-inst 的库）
export MSYSTEM=CLANG64
export MSYS2_PATH_TYPE=inherit
export LC_ALL=C
export HOME=/home/root
LOG=/g/msys64/tmp/build-win32.log
exec > $LOG 2>&1
echo "=== win32 build start $(date) ==="
cd /g/WowType/fcitx5-windows/win32 || exit 1
rm -rf build
cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PREFIX_PATH=/g/WowType/fcitx5-inst 2>&1 | tail -5
cmake --build build 2>&1 | tail -25
echo "=== done $(date) ==="
ls -la build/dll/*.dll 2>/dev/null || echo "no tsf dll"
