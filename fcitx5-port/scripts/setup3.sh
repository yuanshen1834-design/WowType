set -e
write_mirrors() {
  printf 'Server = https://mirrors.ustc.edu.cn/msys2/mingw/clang64/\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/mingw/clang64/\n' > /etc/pacman.d/mirrorlist.clang64
  printf 'Server = https://mirrors.ustc.edu.cn/msys2/mingw/mingw64/\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/mingw/mingw64/\n' > /etc/pacman.d/mirrorlist.mingw64
  printf 'Server = https://mirrors.ustc.edu.cn/msys2/msys/$arch\nServer = https://mirrors.tuna.tsinghua.edu.cn/msys2/msys/$arch\n' > /etc/pacman.d/mirrorlist.msys
}
write_mirrors
echo "--- 源已重写 ---"
pacman -Syy --noconfirm 2>&1 | tail -2
echo "--- Syu ---"
pacman -Syu --noconfirm 2>&1 | tail -3
echo "--- Syu 后重写源（防覆盖）---"
write_mirrors
pacman -Syy --noconfirm 2>&1 | tail -1