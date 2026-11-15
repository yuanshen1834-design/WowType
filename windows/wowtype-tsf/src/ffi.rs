//! wowtype_core.dll 的 C FFI 绑定（手写，对应 core/include/wowtype_core.h）。
//!
//! ABI 1.10：结构体第一个字段都是 size，外壳填 sizeof，核心按它读。

#![allow(non_camel_case_types)]

use std::ffi::c_char;

pub const WOWTYPE_MOD_SHIFT: u32 = 1 << 0;
pub const WOWTYPE_MOD_CAPS: u32 = 1 << 1;
pub const WOWTYPE_MOD_CTRL: u32 = 1 << 2;
pub const WOWTYPE_MOD_ALT: u32 = 1 << 3;
pub const WOWTYPE_MOD_NUMLOCK: u32 = 1 << 4;
pub const WOWTYPE_MOD_SUPER: u32 = 1 << 6;

pub const WOWTYPE_KEY_ACTION_NONE: u32 = 0;
pub const WOWTYPE_KEY_ACTION_PREV_PAGE: u32 = 1;
pub const WOWTYPE_KEY_ACTION_NEXT_PAGE: u32 = 2;
pub const WOWTYPE_KEY_ACTION_PREV_CANDIDATE: u32 = 3;
pub const WOWTYPE_KEY_ACTION_NEXT_CANDIDATE: u32 = 4;
pub const WOWTYPE_KEY_ACTION_OBSERVE: u32 = 5;
pub const WOWTYPE_KEY_ACTION_TOGGLE_MODE: u32 = 6;
pub const WOWTYPE_KEY_ACTION_TOGGLE_TRANSLATE: u32 = 7;

pub const WOWTYPE_MODE_CHINESE: i32 = 0;
pub const WOWTYPE_MODE_ENGLISH: i32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WowtypeKey {
    pub size: u32,
    pub keysym: u32,
    pub modifiers: u32,
    pub release: u32,
    pub prev_char: u32,
    pub action: u32,
    pub hints: u32,
    pub code: u32,
    pub repeat: u32,
    pub time_ms: u64,
}

impl Default for WowtypeKey {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            keysym: 0,
            modifiers: 0,
            release: 0,
            prev_char: 0,
            action: WOWTYPE_KEY_ACTION_NONE,
            hints: 0,
            code: 0,
            repeat: 0,
            time_ms: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WowtypeSessionConfig {
    pub size: u32,
    pub page_size: u32,
    pub calculator: i32,
    pub date_time: i32,
    pub emoji_suggest: i32,
    pub horizontal: i32,
    pub page_keys: u32,
    pub select_keys: i32,
    pub pick_char: i32,
    pub punct_style: i32,
    pub auto_space: u32,
    pub shift_toggle: i32,
    pub shift_tap_ms: u32,
    pub translate_mode: i32,
    pub toolbar: i32,
    pub clipboard_history: i32,
    pub shuangpin_keys: i32,
    pub arrow_keys: i32,
    pub wubi: i32,
    pub wubi_auto_commit: i32,
    pub wubi_hint: i32,
    pub translate_key: i32,
}

impl Default for WowtypeSessionConfig {
    fn default() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as u32,
            page_size: 0,
            calculator: 1,
            date_time: 1,
            emoji_suggest: 1,
            horizontal: 1,
            page_keys: 1, // MINUS
            select_keys: 0,
            pick_char: 1,
            punct_style: 0,
            auto_space: 0,
            shift_toggle: 0,
            shift_tap_ms: 0,
            translate_mode: 0,
            toolbar: 1,
            clipboard_history: 0,
            shuangpin_keys: 0,
            arrow_keys: 0,
            wubi: 0,
            wubi_auto_commit: 1,
            wubi_hint: 1,
            translate_key: 0,
        }
    }
}

#[repr(C)]
pub struct WowtypeFrame {
    pub size: u32,
    pub handled: i32,
    pub commit: *const c_char,
    pub preedit: *const c_char,
    pub preedit_cursor: i32,
    pub aux: *const c_char,
    pub panel: i32,
    pub cand_count: usize,
    pub cand_cursor: i32,
    pub has_prev: i32,
    pub has_next: i32,
    pub wake_after_ms: u32,
    pub forgot: i32,
    pub learned: i32,
    pub mode: i32,
    pub mode_changed: i32,
    pub translating: i32,
    pub translate_source: *const c_char,
    pub translate_text: *const c_char,
    pub translate_title: *const c_char,
    pub translate_status: *const c_char,
    pub translate_done: i32,
}

/// 把 C 字符串指针读成 String（空指针 → 空串）。
pub unsafe fn read_cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}

// ---------- 函数指针类型 ----------

pub type EngineNewFn = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut std::ffi::c_void;
pub type EngineFreeFn = unsafe extern "C" fn(*mut std::ffi::c_void);
pub type EngineSetFuzzyFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> i32;
pub type EngineSetBeamFn = unsafe extern "C" fn(*mut std::ffi::c_void, u32) -> i32;
pub type EngineSetMaxCandidatesFn = unsafe extern "C" fn(*mut std::ffi::c_void, u32) -> i32;
pub type EngineLoadWubiFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char) -> i32;
pub type EngineLoadEnglishFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char) -> i32;
pub type SessionNewFn = unsafe extern "C" fn(
    *mut std::ffi::c_void,
    *const WowtypeSessionConfig,
) -> *mut std::ffi::c_void;
pub type SessionFreeFn = unsafe extern "C" fn(*mut std::ffi::c_void);
pub type SessionKeyFn =
    unsafe extern "C" fn(*mut std::ffi::c_void, *const WowtypeKey) -> *const WowtypeFrame;
pub type SessionFrameFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> *const WowtypeFrame;
pub type SessionResetFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> *const WowtypeFrame;
pub type SessionCandidateFn =
    unsafe extern "C" fn(*mut std::ffi::c_void, usize) -> *const c_char;
pub type SessionSelectFn =
    unsafe extern "C" fn(*mut std::ffi::c_void, usize) -> *const WowtypeFrame;
pub type AbiVersionFn = unsafe extern "C" fn() -> i32;
