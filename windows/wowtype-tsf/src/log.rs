//! 调试日志（性能优化版）。
//!
//! - 详细按键日志默认关闭（设 WOWTYPE_DEBUG=1 才开），避免每键写磁盘
//! - 文件句柄常驻（不反复 open/close）
//! - 重要事件（引擎加载、上屏失败、注册）总是记

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
        Some(dir.join("tsf.log"))
    })
    .clone()
}

/// 详细日志开关（环境变量 WOWTYPE_DEBUG=1）。
fn debug_enabled() -> bool {
    static DEBUG: OnceLock<bool> = OnceLock::new();
    *DEBUG.get_or_init(|| std::env::var("WOWTYPE_DEBUG").map(|v| v == "1").unwrap_or(false))
}

static FILE: Mutex<Option<File>> = Mutex::new(None);

/// 初始化（建目录）。可重复调用。
pub fn init() {
    let _ = log_path();
}

/// 详细日志（按键等）：默认不写，开 WOWTYPE_DEBUG=1 才写。
pub fn write(msg: &str) {
    if !debug_enabled() {
        return;
    }
    write_line(msg);
}

/// 重要日志（引擎加载、上屏失败等）：总是写。
pub fn info(msg: &str) {
    write_line(msg);
}

fn write_line(msg: &str) {
    let Some(path) = log_path() else { return };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let pid = std::process::id();
    let line = format!("[{ts}] [pid={pid}] {msg}\n");
    if let Ok(mut guard) = FILE.lock() {
        if guard.is_none() {
            *guard = OpenOptions::new().create(true).append(true).open(&path).ok();
        }
        if let Some(file) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }
}
