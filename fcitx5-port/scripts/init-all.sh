#!/bin/bash
export MSYSTEM=CLANG64
export MSYS2_PATH_TYPE=inherit
export LC_ALL=C
export HOME=/home/root
LOG=/g/msys64/tmp/init.log
exec > $LOG 2>&1
echo "=== init start $(date) ==="
# 镜像
printf 'Server = https://mirrors.ustc.edu.cn/msys2/mingw/clang64/\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/mingw/clang64/\n' > /etc/pacman.d/mirrorlist.clang64
printf 'Server = https://mirrors.ustc.edu.cn/msys2/mingw/mingw64/\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/mingw/mingw64/\n' > /etc/pacman.d/mirrorlist.mingw64
printf 'Server = https://mirrors.ustc.edu.cn/msys2/msys/$arch\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/msys/$arch\n' > /etc/pacman.d/mirrorlist.msys
pacman-key --init --populate msys2 2>&1 | tail -1
pacman -Syy --noconfirm 2>&1 | tail -1
echo "--- Syu ---"
pacman -Syu --noconfirm 2>&1 | tail -2
echo "--- 重写镜像（防覆盖）---"
printf 'Server = https://mirrors.ustc.edu.cn/msys2/mingw/clang64/\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/mingw/clang64/\n' > /etc/pacman.d/mirrorlist.clang64
printf 'Server = https://mirrors.ustc.edu.cn/msys2/mingw/mingw64/\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/mingw/mingw64/\n' > /etc/pacman.d/mirrorlist.mingw64
printf 'Server = https://mirrors.ustc.edu.cn/msys2/msys/$arch\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/msys/$arch\n' > /etc/pacman.d/mirrorlist.msys
pacman -Syy --noconfirm 2>&1 | tail -1
echo "--- 装依赖 ---"
pacman -S --noconfirm --needed \
  mingw-w64-clang-x86_64-extra-cmake-modules \
  mingw-w64-clang-x86_64-dlfcn \
  mingw-w64-clang-x86_64-libuv \
  mingw-w64-clang-x86_64-clang \
  mingw-w64-clang-x86_64-cmake \
  mingw-w64-clang-x86_64-ninja \
  mingw-w64-clang-x86_64-pkgconf \
  mingw-w64-clang-x86_64-gettext-tools \
  mingw-w64-clang-x86_64-toolchain 2>&1 | tail -2
echo "--- 工具验证 ---"
which cmake ninja clang clang++ 2>&1
clang --version 2>&1 | head -1
echo "=== init done $(date) ==="