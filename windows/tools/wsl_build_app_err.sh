#!/bin/bash
export CARGO_HOME=/root/.cargo
export RUSTUP_HOME=/root/.rustup
export PATH=/root/.cargo/bin:$PATH
cd /mnt/g/WowType/windows/wowtype-app
cargo build --release --target x86_64-pc-windows-gnu 2>&1 | grep -A6 'E0432' | head -20
