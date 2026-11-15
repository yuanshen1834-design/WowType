//! COM 对象：类厂 + 文本服务。
//!
//! v13 稳定版：
//! - 无内联组句（拼音显示在候选窗，上屏直接插入——修复跨线程组句 bug）
//! - 编辑键只在有编辑区时宣布吃（修复退格/回车被吞 bug）
//! - 失焦/Deactivate 清理会话（修复状态串档）

use std::cell::Cell;

use crate::log;
use crate::{engine, keys, lock_module, unlock_module};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfThreadMgr,
};
use windows::core::{
    implement, ComObject, GUID, IUnknown, IUnknownImpl, Interface, Ref, Result, BOOL,
};

thread_local! {
    /// 当前线程的 client id（Activate 的 tid），编辑会话要用。
    static CLIENT_ID: Cell<u32> = const { Cell::new(0) };
}

/// 类厂：TSF 通过它创建文本服务对象。
#[implement(windows::Win32::System::Com::IClassFactory)]
pub struct ClassFactory;

impl windows::Win32::System::Com::IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut std::ffi::c_void,
    ) -> Result<()> {
        if !punkouter.is_null() {
            return windows::Win32::Foundation::CLASS_E_NOAGGREGATION.ok();
        }
        if riid.is_null() || ppvobject.is_null() {
            return windows::Win32::Foundation::E_FAIL.ok();
        }
        let service = TextService::new();
        let unknown: IUnknown = service.to_interface::<IUnknown>();
        unsafe { unknown.query(riid, ppvobject).ok() }
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            lock_module();
        } else {
            unlock_module();
        }
        Ok(())
    }
}

/// 文本服务对象（TSF 每个线程一个）。
#[implement(ITfTextInputProcessor, ITfKeyEventSink)]
pub struct TextService {
    client_id: Cell<u32>,
}

impl TextService {
    pub fn new() -> ComObject<Self> {
        lock_module();
        ComObject::new(Self {
            client_id: Cell::new(0),
        })
    }
}

impl Drop for TextService {
    fn drop(&mut self) {
        unlock_module();
    }
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        self.client_id.set(tid);
        CLIENT_ID.with(|c| c.set(tid));
        let thread_mgr = match ptim.ok() {
            Ok(t) => t.clone(),
            Err(e) => {
                log::info(&format!("Activate 拿不到 ITfThreadMgr: {e:?}"));
                return Err(e);
            }
        };
        let keystroke: ITfKeystrokeMgr = thread_mgr.cast()?;
        let sink: ITfKeyEventSink = self.to_interface::<ITfKeyEventSink>();
        unsafe { keystroke.AdviseKeyEventSink(tid, &sink, true) }?;
        let object = self.to_object();
        crate::ACTIVE.with(|a| *a.borrow_mut() = Some(std::mem::ManuallyDrop::new(object)));
        Ok(())
    }

    fn Deactivate(&self) -> Result<()> {
        // 释放会话（正常路径主动释放，不依赖 TLS 析构顺序）
        engine::release();
        crate::candidate::hide();
        crate::ACTIVE.with(|a| {
            let _ = a.borrow_mut().take();
        });
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, fforeground: BOOL) -> Result<()> {
        if !fforeground.as_bool() {
            // 失焦：清空拼音缓冲 + 收起候选条（防状态串到别的窗口）
            crate::candidate::hide();
            engine::with_session(|s| {
                let _ = s.reset(false);
            });
        }
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        _pic: Ref<ITfContext>,
        wparam: windows::Win32::Foundation::WPARAM,
        _lparam: windows::Win32::Foundation::LPARAM,
    ) -> Result<BOOL> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            on_test_key_down(wparam.0 as u32)
        }));
        match result {
            Ok(eat) => Ok(BOOL(if eat { 1 } else { 0 })),
            Err(_) => {
                log::info("OnTestKeyDown panic（已兜住）");
                Ok(BOOL(0))
            }
        }
    }

    fn OnKeyDown(
        &self,
        pic: Ref<ITfContext>,
        wparam: windows::Win32::Foundation::WPARAM,
        _lparam: windows::Win32::Foundation::LPARAM,
    ) -> Result<BOOL> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            on_key_down(&pic, wparam.0 as u32)
        }));
        match result {
            Ok(handled) => Ok(BOOL(if handled { 1 } else { 0 })),
            Err(_) => {
                log::info("OnKeyDown panic（已兜住，放行按键）");
                Ok(BOOL(0))
            }
        }
    }

    fn OnTestKeyUp(
        &self,
        _pic: Ref<ITfContext>,
        wparam: windows::Win32::Foundation::WPARAM,
        _lparam: windows::Win32::Foundation::LPARAM,
    ) -> Result<BOOL> {
        let vk = wparam.0 as u32;
        let keysym = keys::vk_to_keysym(vk);
        if keysym == keys::XK_SHIFT_L || keysym == keys::XK_SHIFT_R {
            return Ok(BOOL(1));
        }
        Ok(BOOL(0))
    }

    fn OnKeyUp(
        &self,
        pic: Ref<ITfContext>,
        wparam: windows::Win32::Foundation::WPARAM,
        _lparam: windows::Win32::Foundation::LPARAM,
    ) -> Result<BOOL> {
        let vk = wparam.0 as u32;
        let keysym = keys::vk_to_keysym(vk);
        if keysym != keys::XK_SHIFT_L && keysym != keys::XK_SHIFT_R {
            return Ok(BOOL(0));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            on_shift_release(&pic, keysym)
        }));
        match result {
            Ok(handled) => Ok(BOOL(if handled { 1 } else { 0 })),
            Err(_) => {
                log::info("OnKeyUp panic（已兜住）");
                Ok(BOOL(0))
            }
        }
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}

/// OnTestKeyDown 的判定。
///
/// 重要（TSF 语义）：宣布吃（TRUE）的键不会再传给应用。
/// 所以：只有"引擎会处理"或"能无损回插字符"的键才吃，其余一律放行。
fn on_test_key_down(vk: u32) -> bool {
    let keysym = keys::vk_to_keysym(vk);
    if keysym == 0 {
        return false;
    }
    let mods = keys::current_modifiers();
    // 带 Ctrl/Alt/Win（Shift 除外）的组合放行给应用
    if mods & (crate::ffi::WOWTYPE_MOD_CTRL | crate::ffi::WOWTYPE_MOD_ALT | crate::ffi::WOWTYPE_MOD_SUPER)
        != 0
    {
        return false;
    }
    // Shift：单按切换要接，吃
    if keysym == keys::XK_SHIFT_L || keysym == keys::XK_SHIFT_R {
        return true;
    }
    // 可打印字符（字母/数字/标点/空格）：能回插，吃
    if keys::keysym_to_char(keysym, mods).is_some() {
        return true;
    }
    // 编辑键（退格/回车/Esc/Tab/方向等）：仅当有拼音编辑区时才吃
    // （否则放行给应用——修复"退格永远删不掉字符"的 bug）
    let editing = engine::with_session(|s| !s.frame_preedit().is_empty()).unwrap_or(false);
    editing
}

/// OnKeyDown 的实际处理（被 catch_unwind 包裹）。返回是否吃掉按键。
fn on_key_down(pic: &Ref<ITfContext>, vk: u32) -> bool {
    let Some(ctx) = pic.cloned() else {
        return false;
    };
    let keysym = keys::vk_to_keysym(vk);
    if keysym == 0 {
        return false;
    }
    if !engine::ready() {
        engine::Session::ensure();
    }
    let modifiers = keys::current_modifiers();
    let key = crate::ffi::WowtypeKey {
        size: std::mem::size_of::<crate::ffi::WowtypeKey>() as u32,
        keysym,
        modifiers,
        release: 0,
        prev_char: 0,
        action: crate::ffi::WOWTYPE_KEY_ACTION_NONE,
        hints: 0,
        code: 0,
        repeat: 0,
        time_ms: 0,
    };
    let tid = CLIENT_ID.with(|c| c.get());
    match engine::key(key) {
        Some(frame) => {
            log::write(&format!(
                "键 {:#x} handled={} preedit={:?} commit={:?} cand={} mode={}",
                keysym, frame.handled, frame.preedit, frame.commit, frame.cand_count, frame.mode
            ));
            if frame.mode_changed {
                log::info(&format!(
                    "切换输入模式 → {}",
                    if frame.mode == crate::ffi::WOWTYPE_MODE_ENGLISH {
                        "英文"
                    } else {
                        "中文"
                    }
                ));
                crate::candidate::hide();
            }
            // 上屏（无内联组句：直接插入）
            if !frame.commit.is_empty() {
                let result = crate::itf::commit_text(&ctx, tid, frame.commit.clone());
                if let Err(e) = result {
                    log::info(&format!("上屏失败: {e:?}"));
                }
            }
            // 候选窗：拼音 + 候选（最多 100 个 clamp）
            if frame.cand_count > 0 || !frame.preedit.is_empty() {
                let count = frame.cand_count.min(100);
                let items: Vec<String> = (0..count)
                    .filter_map(|i| engine::with_session(|s| s.candidate(i)).flatten())
                    .collect();
                crate::candidate::show(&frame.preedit, items, frame.cand_cursor, frame.has_prev, frame.has_next);
            } else {
                crate::candidate::hide();
            }
            if frame.handled {
                true
            } else {
                // 会话放行：自己把字符插回（保证键不丢）
                match keys::keysym_to_char(keysym, modifiers) {
                    Some(c) => {
                        let result =
                            crate::itf::commit_text(&ctx, tid, c.to_string());
                        if let Err(e) = result {
                            log::info(&format!("回插字符失败: {e:?}"));
                        }
                        true
                    }
                    None => false,
                }
            }
        }
        None => {
            log::info("引擎不可用，放行按键");
            false
        }
    }
}

/// Shift 松开：交给会话做「单按 Shift 切换中英文」判定。返回是否吃掉。
fn on_shift_release(pic: &Ref<ITfContext>, keysym: u32) -> bool {
    let Some(ctx) = pic.cloned() else {
        return false;
    };
    if !engine::ready() {
        engine::Session::ensure();
    }
    let key = crate::ffi::WowtypeKey {
        size: std::mem::size_of::<crate::ffi::WowtypeKey>() as u32,
        keysym,
        modifiers: keys::current_modifiers(),
        release: 1,
        prev_char: 0,
        action: crate::ffi::WOWTYPE_KEY_ACTION_NONE,
        hints: 0,
        code: 0,
        repeat: 0,
        time_ms: 0,
    };
    let tid = CLIENT_ID.with(|c| c.get());
    match engine::key(key) {
        Some(frame) => {
            if frame.mode_changed {
                log::info(&format!(
                    "单按 Shift 切换 → {}",
                    if frame.mode == crate::ffi::WOWTYPE_MODE_ENGLISH {
                        "英文"
                    } else {
                        "中文"
                    }
                ));
                crate::candidate::hide();
            }
            if !frame.commit.is_empty() {
                let _ = crate::itf::commit_text(&ctx, tid, frame.commit.clone());
            }
            true
        }
        None => false,
    }
}
