#!/bin/bash
# 在 WSL 里交叉编译 Windows TSF DLL
set -e
export CARGO_HOME=/root/.cargo
export RUSTUP_HOME=/root/.rustup
export PATH=/root/.cargo/bin:$PATH
cd /mnt/g/WowType/windows/wowtype-tsf
echo '=== 编译（target x86_64-pc-windows-gnu）==='
cargo build --release --target x86_64-pc-windows-gnu 2>&1 | tail -40
echo '=== 产物 ==='
ls -la target/x86_64-pc-windows-gnu/release/*.dll 2>/dev/null || echo '无 DLL 产物'
