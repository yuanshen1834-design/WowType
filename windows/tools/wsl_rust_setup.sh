#!/bin/bash
# WSL 里安装 Rust + Windows 交叉编译目标
set -e
cd /tmp
export RUSTUP_DIST_SERVER=https://rsproxy.cn
export RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
export CARGO_HOME=/root/.cargo
export RUSTUP_HOME=/root/.rustup

if [ ! -x /root/.cargo/bin/rustc ]; then
  echo '=== 下载 rustup-init ==='
  curl -sSL --retry 3 -o /tmp/rustup-init https://rsproxy.cn/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
  chmod +x /tmp/rustup-init
  echo '=== 安装 Rust ==='
  /tmp/rustup-init -y --profile minimal --default-host x86_64-unknown-linux-gnu --no-modify-path
fi

echo '=== 添加 Windows GNU 目标 ==='
/root/.cargo/bin/rustup target add x86_64-pc-windows-gnu
echo '=== 配置交叉链接器 ==='
mkdir -p /root/.cargo
cat > /root/.cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-ar"
EOF
echo '=== 验证 ==='
/root/.cargo/bin/rustc --version
/root/.cargo/bin/cargo --version
/root/.cargo/bin/rustup target list --installed
