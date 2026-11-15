#!/bin/bash
# WSL 里安装 Rust（官方源直连）+ Windows 交叉编译目标
set -e
export CARGO_HOME=/root/.cargo
export RUSTUP_HOME=/root/.rustup
export RUSTUP_DIST_SERVER=https://static.rust-lang.org
export RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup

if [ ! -f /root/.rustup/settings.toml ] || [ ! -x /root/.cargo/bin/rustc ]; then
  echo '=== 安装 Rust ==='
  /tmp/rustup-init -y --profile minimal --default-host x86_64-unknown-linux-gnu --no-modify-path 2>&1 | tail -6
fi

echo '=== 添加 Windows GNU 目标 ==='
/root/.cargo/bin/rustup target add x86_64-pc-windows-gnu 2>&1 | tail -3

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
/root/.cargo/bin/rustc --version || true
/root/.cargo/bin/cargo --version || true
/root/.cargo/bin/rustup target list --installed 2>/dev/null || true
