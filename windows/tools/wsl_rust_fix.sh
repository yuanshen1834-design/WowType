#!/bin/bash
# 下载 stable 工具链（static.rust-lang.org 直连）
set -e
export CARGO_HOME=/root/.cargo
export RUSTUP_HOME=/root/.rustup
export RUSTUP_DIST_SERVER=https://static.rust-lang.org
export RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup
export PATH=/root/.cargo/bin:$PATH

echo '=== 设置默认工具链（会下载）==='
rustup default stable 2>&1 | tail -8
echo '=== 添加 Windows GNU 目标 ==='
rustup target add x86_64-pc-windows-gnu 2>&1 | tail -3
echo '=== 验证 ==='
rustc --version
cargo --version
rustup target list --installed
