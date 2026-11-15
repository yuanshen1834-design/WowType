//! 引擎管理（独立 App 版）：加载 wowtype_core.dll + 词库，管理会话。
//!
//! 与 TSF 版不同：这里进程唯一，全局一份引擎，不存在多进程重复加载问题。

use std::cell::RefCell;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::{Result, PCWSTR};
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetProcAddress, LoadLibraryW};

use crate::ffi::*;
use crate::log;

/// exe 所在目录。
pub fn exe_dir() -> PathBuf {
    let mut buf = [0u16; 1024];
    let len = unsafe { GetModuleFileNameW(None, &mut buf) } as usize;
    let path = String::from_utf16_lossy(&buf[..len]);
    let mut p = PathBuf::from(path);
    p.pop();
    p
}

/// 数据文件：exe 同目录 data/ 下。
pub fn data_path(name: &str) -> Option<PathBuf> {
    let mut p = exe_dir();
    p.push("data");
    p.push(name);
    Some(p)
}

pub struct CoreApi {
    pub abi_version: AbiVersionFn,
    pub engine_new: EngineNewFn,
    pub engine_free: EngineFreeFn,
    pub engine_set_fuzzy: EngineSetFuzzyFn,
    pub engine_set_beam_size: EngineSetBeamFn,
    pub engine_set_max_candidates: EngineSetMaxCandidatesFn,
    pub session_new: SessionNewFn,
    pub session_free: SessionFreeFn,
    pub session_key: SessionKeyFn,
    pub session_frame: SessionFrameFn,
    pub session_reset: SessionResetFn,
    pub session_candidate: SessionCandidateFn,
    pub session_select: SessionSelectFn,
}

impl CoreApi {
    pub fn load() -> Result<Self> {
        let mut path = exe_dir();
        path.push("wowtype_core.dll");
        log::info(&format!("加载 core: {}", path.display()));
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let module = unsafe { LoadLibraryW(PCWSTR(wide.as_ptr()))? };

        unsafe fn sym<T>(module: windows::Win32::Foundation::HMODULE, name: &str) -> T {
            let name_c = std::ffi::CString::new(name).unwrap();
            let proc = GetProcAddress(module, windows::core::PCSTR(name_c.as_ptr() as *const u8))
                .expect("missing export");
            std::mem::transmute_copy(&proc)
        }

        let api = Self {
            abi_version: unsafe { sym(module, "wowtype_abi_version") },
            engine_new: unsafe { sym(module, "wowtype_engine_new") },
            engine_free: unsafe { sym(module, "wowtype_engine_free") },
            engine_set_fuzzy: unsafe { sym(module, "wowtype_engine_set_fuzzy") },
            engine_set_beam_size: unsafe { sym(module, "wowtype_engine_set_beam_size") },
            engine_set_max_candidates: unsafe { sym(module, "wowtype_engine_set_max_candidates") },
            session_new: unsafe { sym(module, "wowtype_session_new") },
            session_free: unsafe { sym(module, "wowtype_session_free") },
            session_key: unsafe { sym(module, "wowtype_session_key") },
            session_frame: unsafe { sym(module, "wowtype_session_frame") },
            session_reset: unsafe { sym(module, "wowtype_session_reset") },
            session_candidate: unsafe { sym(module, "wowtype_session_candidate") },
            session_select: unsafe { sym(module, "wowtype_session_select") },
        };
        log::info(&format!("core ABI {}", unsafe { (api.abi_version)() }));
        Ok(api)
    }
}

pub struct Session {
    handle: *mut c_void,
}

thread_local! {
    static CORE: RefCell<Option<CoreApi>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

impl Session {
    /// 准备会话（全局单例）。
    pub fn ensure() -> bool {
        SESSION.with(|s| {
            if s.borrow().is_some() {
                return true;
            }
            match Self::create() {
                Some(session) => {
                    *s.borrow_mut() = Some(session);
                    true
                }
                None => false,
            }
        })
    }

    fn create() -> Option<Session> {
        CORE.with(|c| {
            let mut cache = c.borrow_mut();
            if cache.is_none() {
                match CoreApi::load() {
                    Ok(api) => *cache = Some(api),
                    Err(e) => {
                        log::info(&format!("加载 core 失败: {e:?}"));
                        return None;
                    }
                }
            }
            let api = cache.as_ref()?;
            let lexicon_path = data_path("lexicon.tsv")?;
            let lexicon = std::ffi::CString::new(lexicon_path.to_string_lossy().to_string()).ok()?;
            // 语言模型可选（WOWTYPE_NO_LM=1 不加载 → 省内存，整句质量略降）
            let lm = if std::env::var("WOWTYPE_NO_LM").is_ok() {
                None
            } else {
                let lm_path = data_path("words.lm")?;
                Some(std::ffi::CString::new(lm_path.to_string_lossy().to_string()).ok()?)
            };
            let lm_ptr = lm.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
            let t0 = std::time::Instant::now();
            let engine = unsafe { (api.engine_new)(lexicon.as_ptr(), lm_ptr) };
            if engine.is_null() {
                log::info("引擎创建失败（词库缺失？）");
                return None;
            }
            log::info(&format!(
                "引擎加载耗时 {} ms（语言模型: {}）",
                t0.elapsed().as_millis(),
                if lm.is_some() { "开" } else { "关" }
            ));
            // 候选要多（生僻字、同音词），最多 100 个：每页 9 个，可翻页
            unsafe {
                (api.engine_set_max_candidates)(engine, 100);
                (api.engine_set_beam_size)(engine, 12);
                (api.engine_set_fuzzy)(engine, 0);
            }
            let mut config = WowtypeSessionConfig::default();
            config.page_size = 9;
            let session = unsafe { (api.session_new)(engine, &config) };
            if session.is_null() {
                log::info("会话创建失败");
                unsafe { (api.engine_free)(engine) };
                return None;
            }
            log::info("引擎 + 会话就绪（最多 100 候选）");
            Some(Session { handle: session })
        })
    }

    pub fn key(&self, key: &WowtypeKey) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_key)(self.handle, key) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }

    pub fn reset(&self, commit_pending: bool) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_reset)(self.handle, commit_pending as i32) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }

    pub fn frame(&self) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_frame)(self.handle) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }

    pub fn candidate(&self, index: usize) -> Option<String> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let ptr = unsafe { (api.session_candidate)(self.handle, index) };
            if ptr.is_null() {
                None
            } else {
                Some(unsafe { read_cstr(ptr) })
            }
        })
    }

    pub fn select(&self, index: usize) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_select)(self.handle, index) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }
}

#[derive(Default, Clone)]
pub struct FrameSnapshot {
    pub handled: bool,
    pub commit: String,
    pub preedit: String,
    pub preedit_cursor: i32,
    pub aux: String,
    pub panel: i32,
    pub cand_count: usize,
    pub cand_cursor: i32,
    pub has_prev: bool,
    pub has_next: bool,
    pub mode: i32,
    pub mode_changed: bool,
}

impl FrameSnapshot {
    unsafe fn from_frame(frame: *const WowtypeFrame) -> Option<Self> {
        if frame.is_null() {
            return None;
        }
        let f = unsafe { &*frame };
        Some(Self {
            handled: f.handled != 0,
            commit: unsafe { read_cstr(f.commit) },
            preedit: unsafe { read_cstr(f.preedit) },
            preedit_cursor: f.preedit_cursor,
            aux: unsafe { read_cstr(f.aux) },
            panel: f.panel,
            cand_count: f.cand_count,
            cand_cursor: f.cand_cursor,
            has_prev: f.has_prev != 0,
            has_next: f.has_next != 0,
            mode: f.mode,
            mode_changed: f.mode_changed != 0,
        })
    }
}

pub fn key(key: WowtypeKey) -> Option<FrameSnapshot> {
    SESSION.with(|s| {
        let session = s.borrow();
        session.as_ref()?.key(&key)
    })
}

pub fn ready() -> bool {
    SESSION.with(|s| s.borrow().is_some())
}

pub fn with_session<R>(f: impl FnOnce(&Session) -> R) -> Option<R> {
    SESSION.with(|s| {
        let session = s.borrow();
        session.as_ref().map(f)
    })
}

/// 当前编辑区文字。
pub fn preedit() -> String {
    with_session(|s| s.frame().map(|f| f.preedit)).flatten().unwrap_or_default()
}

/// 当前输入模式。
pub fn mode() -> i32 {
    with_session(|s| s.frame().map(|f| f.mode))
        .flatten()
        .unwrap_or(WOWTYPE_MODE_CHINESE)
}
