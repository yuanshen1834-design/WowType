//! 加载 wowtype_core.dll 并管理引擎 / 会话（进程内，P1 简化版）。
//!
//! 每个宿主线程一个会话（TSF 每线程一个 TextService）。
//! 引擎在第一次需要时加载词库，之后该线程复用。
//! 数据文件放在 TSF DLL 同目录的 data/ 下（便于打包分发）。

use std::cell::RefCell;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;

use windows::core::{Result, PCWSTR};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use crate::ffi::*;
use crate::log;

/// 数据文件路径：TSF DLL 同目录的 data/ 下。
fn data_path(name: &str) -> Option<String> {
    let module = crate::module_path().ok()?;
    let mut path = std::path::PathBuf::from(module.to_string());
    path.pop();
    path.push("data");
    path.push(name);
    Some(path.to_string_lossy().into_owned())
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
    /// 从 TSF DLL 同目录加载 wowtype_core.dll。
    pub fn load() -> Result<Self> {
        let module = crate::module_path()?;
        let mut path = std::path::PathBuf::from(module.to_string());
        path.pop();
        path.push("wowtype_core.dll");
        log::write(&format!("加载 core: {}", path.display()));

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

        // 路径来自模块自身目录；函数名与 ABI 头文件一致。
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
        log::write(&format!("core ABI {}", unsafe { (api.abi_version)() }));
        Ok(api)
    }
}

/// 一个会话（单线程用）。
pub struct Session {
    handle: *mut c_void,
}

// 会话只在创建它的线程里用（TSF 保证）。
unsafe impl Send for Session {}

thread_local! {
    static CORE: RefCell<Option<CoreApi>> = const { RefCell::new(None) };
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

impl Session {
    /// 为当前线程准备会话（已存在则复用）。
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
                        log::write(&format!("加载 core 失败: {e:?}"));
                        return None;
                    }
                }
            }
            let api = cache.as_ref()?;
            let lexicon_path = {
                let small = data_path("lexicon_small.tsv");
                match small {
                    Some(p) if std::path::Path::new(&p).exists() => p,
                    _ => data_path("lexicon.tsv")?,
                }
            };
            let lm_path = data_path("words.lm")?;
            crate::log::info(&format!("词库: {lexicon_path}"));
            let lexicon = std::ffi::CString::new(lexicon_path).ok()?;
            let lm = std::ffi::CString::new(lm_path).ok()?;
            let engine = unsafe { (api.engine_new)(lexicon.as_ptr(), lm.as_ptr()) };
            if engine.is_null() {
                crate::log::info("引擎创建失败（词库路径不对？）");
                return None;
            }
            // 性能：减少每键的搜索宽度和输出候选数（默认值偏大，打字会慢）
            unsafe {
                (api.engine_set_max_candidates)(engine, 100);
                (api.engine_set_beam_size)(engine, 8);
                (api.engine_set_fuzzy)(engine, 0);
            }
            let mut config = WowtypeSessionConfig::default();
            config.page_size = 9;
            let session = unsafe { (api.session_new)(engine, &config) };
            if session.is_null() {
                crate::log::info("会话创建失败");
                unsafe { (api.engine_free)(engine) };
                return None;
            }
            crate::log::info("引擎 + 会话就绪");
            Some(Session { handle: session })
        })
    }

    /// 处理一次按键，返回 frame 的快照（String 已拷贝，指针失效也没关系）。
    pub fn key(&self, key: &WowtypeKey) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_key)(self.handle, key) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }

    /// 清空编辑区。
    pub fn reset(&self, commit_pending: bool) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_reset)(self.handle, commit_pending as i32) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }

    /// 当前编辑区文字（不处理按键）。
    pub fn frame(&self) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_frame)(self.handle) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }

    /// 当前编辑区文字（简化）。
    pub fn frame_preedit(&self) -> String {
        self.frame().map(|f| f.preedit).unwrap_or_default()
    }

    /// 第 index 个候选文字。
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

    /// 鼠标选中第 index 个候选。
    pub fn select(&self, index: usize) -> Option<FrameSnapshot> {
        CORE.with(|c| {
            let cache = c.borrow();
            let api = cache.as_ref()?;
            let frame = unsafe { (api.session_select)(self.handle, index) };
            unsafe { FrameSnapshot::from_frame(frame) }
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // try_with：TLS 可能已销毁（线程退出），绝不能在析构器里 panic
        let _ = CORE.try_with(|c| {
            if let Some(api) = c.borrow().as_ref() {
                unsafe { (api.session_free)(self.handle) };
            }
        });
    }
}

/// 显式释放当前线程的会话（Deactivate 时调用，不依赖 TLS 析构顺序）。
pub fn release() {
    SESSION.with(|s| {
        let _ = s.borrow_mut().take();
    });
}

/// 对 WowtypeFrame 的快照（字符串已转成 Rust String）。
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
        // ABI 校验：size 必须覆盖到 cand_count 字段（防版本错位读垃圾值）
        let min_size = std::mem::offset_of!(WowtypeFrame, cand_count) + std::mem::size_of::<usize>();
        if (f.size as usize) < min_size {
            return None;
        }
        Some(Self {
            handled: f.handled != 0,
            commit: unsafe { read_cstr(f.commit) },
            preedit: unsafe { read_cstr(f.preedit) },
            preedit_cursor: f.preedit_cursor,
            aux: unsafe { read_cstr(f.aux) },
            panel: f.panel,
            cand_count: f.cand_count.min(200),
            cand_cursor: f.cand_cursor,
            has_prev: f.has_prev != 0,
            has_next: f.has_next != 0,
            mode: f.mode,
            mode_changed: f.mode_changed != 0,
        })
    }
}

/// 对当前线程的会话做一次按键处理。
pub fn key(key: WowtypeKey) -> Option<FrameSnapshot> {
    SESSION.with(|s| {
        let session = s.borrow();
        session.as_ref()?.key(&key)
    })
}

/// 当前线程是否有会话。
pub fn ready() -> bool {
    SESSION.with(|s| s.borrow().is_some())
}

/// 取当前会话（用于候选列表）。
pub fn with_session<R>(f: impl FnOnce(&Session) -> R) -> Option<R> {
    SESSION.with(|s| {
        let session = s.borrow();
        session.as_ref().map(f)
    })
}
