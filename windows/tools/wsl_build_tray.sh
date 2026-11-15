#!/bin/bash
export CARGO_HOME=/root/.cargo
export RUSTUP_HOME=/root/.rustup
export PATH=/root/.cargo/bin:$PATH
cd /mnt/g/WowType/windows/wowtype-tray
cargo build --release --target x86_64-pc-windows-gnu 2>&1 | tail -30
echo '=== 产物 ==='
ls -la target/x86_64-pc-windows-gnu/release/*.exe 2>/dev/null || echo '无 exe'
