//! 日志（独立 App 版）：%LOCALAPPDATA%\WowType\logs\app.log。
//! 默认只记重要事件；WOWTYPE_DEBUG=1 时记详细。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn log_path() -> Option<PathBuf> {
    static PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    PATH.get_or_init(|| {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("TEMP").map(PathBuf::from))?;
        let dir = base.join("WowType").join("logs");
        let _ = std::fs::create_dir_all(&dir);
        Some(dir.join("app.log"))
    })
    .clone()
}

fn debug_enabled() -> bool {
    static DEBUG: OnceLock<bool> = OnceLock::new();
    *DEBUG.get_or_init(|| std::env::var("WOWTYPE_DEBUG").map(|v| v == "1").unwrap_or(false))
}

static FILE: Mutex<Option<File>> = Mutex::new(None);

pub fn init() {
    let _ = log_path();
}

/// 详细日志（默认关闭）。
pub fn write(msg: &str) {
    if !debug_enabled() {
        return;
    }
    write_line(msg);
}

/// 重要日志（总是写）。
pub fn info(msg: &str) {
    write_line(msg);
}

fn write_line(msg: &str) {
    let Some(path) = log_path() else { return };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("[{ts}] {msg}\n");
    if let Ok(mut guard) = FILE.lock() {
        if guard.is_none() {
            *guard = OpenOptions::new().create(true).append(true).open(&path).ok();
        }
        if let Some(file) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }
}
