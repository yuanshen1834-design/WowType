#!/bin/bash
# 一键构建剩余组件：WinUI addon + WowType addon
export MSYSTEM=CLANG64
export MSYS2_PATH_TYPE=inherit
export LC_ALL=C
export HOME=/home/root
LOG=/g/msys64/tmp/build-addons.log
exec > $LOG 2>&1
echo "=== addons build start $(date) ==="

# 1. WinUI addon（主构建增量）
echo "--- winui ---"
cd /g/WowType/fcitx5-windows || exit 1
cmake --build build/x86_64 2>&1 | tail -5
ls -la build/x86_64/src/winui/*.dll 2>/dev/null

# 2. WowType addon
echo "--- wowtype addon ---"
cd /g/WowType/src/WowType-main/fcitx5-addon || exit 1
rm -rf build-win
cmake -B build-win -G Ninja -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_PREFIX_PATH=/g/WowType/fcitx5-inst \
    -DCMAKE_INSTALL_PREFIX=/g/WowType/fcitx5-inst \
    -DWOWTYPE_CORE_LIB=/g/WowType/src/WowType-main/target/x86_64-pc-windows-gnu/release/libwowtype_core.a \
    -DWOWTYPE_CORE_STATIC=ON \
    -DWOWTYPE_VOICE=OFF \
    -DWOWTYPE_INSTALL_THEMES=OFF \
    -DWOWTYPE_INSTALL_SETTINGS=OFF \
    -DFCITX_INSTALL_ADDONDIR=lib/fcitx5 \
    -DFCITX_INSTALL_PKGDATADIR=share/fcitx5 \
    -DWOWTYPE_SYSTEM_DATA_DIR=G:/WowType/fcitx5-inst/share/wowtype 2>&1 | tail -8
cmake --build build-win 2>&1 | tail -15
ls -la build-win/*.dll 2>/dev/null
echo "=== done $(date) ==="
