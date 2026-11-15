#![windows_subsystem = "windows"]
//! WowType 独立版拼音输入法（不注入系统进程）。
//!
//! 架构：
//! - 全局键盘钩子（WH_KEYBOARD_LL）捕获按键
//! - core 引擎（wowtype_core.dll + 词库）跑在本进程
//! - 候选窗口（仿搜狗：拼音 + 候选 + 翻页）
//! - 状态条（悬浮小窗：中/英切换、可拖动、右键菜单）
//! - 上屏：SendInput（Unicode）

mod candidate;
mod engine;
mod ffi;
mod hook;
mod input;
mod keys;
mod log;
mod statusbar;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, MSG, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn main() {
    log::init();
    log::info("WowType 独立版启动");

    // 预加载引擎（失败也能跑，只提示）
    if !engine::Session::ensure() {
        log::info("引擎加载失败——请确认 data/lexicon.tsv 存在");
    }

    // 状态条
    statusbar::create();
    // 全局键盘钩子
    if !hook::install(handle_key) {
        log::info("钩子安装失败，退出");
        return;
    }

    // 消息循环（钩子回调在此线程执行）
    unsafe {
        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 <= 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    log::info("WowType 退出");
}

/// 全局键盘钩子回调：返回 true 吃掉按键。
fn handle_key(vk: u32, is_up: bool) -> bool {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| process_key(vk, is_up)));
    result.unwrap_or(false)
}

/// 手动模式（三态）：0 自动（按键盘布局判断）、1 强制启用、2 强制暂停。
static FORCE_MODE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// 当前是否激活了别的中文输入法（根据键盘布局语言判断）。
fn other_ime_active() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    unsafe {
        let hkl = GetKeyboardLayout(0);
        let langid = (hkl.0 as u32) & 0xFFFF;
        // 简体中文 0x0804 / 繁体中文 0x0404 + 0x0C04 + 中文香港 0x1004
        matches!(langid, 0x0804 | 0x0404 | 0x0C04 | 0x1004)
    }
}

fn process_key(vk: u32, is_up: bool) -> bool {
    use std::sync::atomic::Ordering;
    let keysym = keys::vk_to_keysym(vk);
    if keysym == 0 {
        return false;
    }
    let modifiers = keys::current_modifiers();

    // 热键 Ctrl+Alt+W：循环切换 自动 → 强制启用 → 强制暂停 → 自动
    if vk == 0x57
        && modifiers & ffi::WOWTYPE_MOD_CTRL != 0
        && modifiers & ffi::WOWTYPE_MOD_ALT != 0
    {
        if !is_up {
            let mode = (FORCE_MODE.load(Ordering::SeqCst) + 1) % 3;
            FORCE_MODE.store(mode, Ordering::SeqCst);
            let (desc, enabled) = match mode {
                0 => ("自动（中文布局让给系统输入法）", !other_ime_active()),
                1 => ("强制启用（WowType 接管）", true),
                _ => ("强制暂停（让给系统输入法）", false),
            };
            log::info(&format!("Ctrl+Alt+W → {desc}"));
            statusbar::set_enabled(enabled);
            if !enabled {
                candidate::hide();
            }
        }
        return true;
    }

    // 带 Ctrl/Alt/Win 的组合放行（系统快捷键）
    if modifiers & (ffi::WOWTYPE_MOD_CTRL | ffi::WOWTYPE_MOD_ALT | ffi::WOWTYPE_MOD_SUPER) != 0 {
        return false;
    }

    // 模式判定：强制暂停 → 放行；强制启用 → 接管；自动 → 中文布局让给系统输入法
    let mode = FORCE_MODE.load(Ordering::SeqCst);
    let should_pause = match mode {
        2 => true,
        1 => false,
        _ => other_ime_active() && engine::preedit().is_empty(),
    };
    if should_pause {
        candidate::hide();
        return false;
    }

    // Shift 松开：交给会话做单按切换判定
    if is_up {
        if keysym == keys::XK_SHIFT_L || keysym == keys::XK_SHIFT_R {
            let key = make_key(keysym, modifiers, true);
            if let Some(frame) = engine::key(key) {
                if frame.mode_changed {
                    statusbar::set_mode(frame.mode);
                    log::info(&format!(
                        "单按 Shift 切换 → {}",
                        if frame.mode == ffi::WOWTYPE_MODE_ENGLISH { "英文" } else { "中文" }
                    ));
                    candidate::hide();
                }
                if !frame.commit.is_empty() {
                    input::commit(&frame.commit);
                }
                return true;
            }
        }
        return false;
    }

    // 按下：可打印字符 + 编辑键交给引擎
    let is_edit_key = matches!(
        keysym,
        keys::XK_BACKSPACE
            | keys::XK_RETURN
            | keys::XK_ESCAPE
            | keys::XK_SPACE
            | keys::XK_TAB
            | keys::XK_DELETE
            | keys::XK_LEFT
            | keys::XK_RIGHT
            | keys::XK_UP
            | keys::XK_DOWN
            | keys::XK_HOME
            | keys::XK_END
            | keys::XK_PAGE_UP
            | keys::XK_PAGE_DOWN
    );
    let is_char = keys::keysym_to_char(keysym, modifiers).is_some();
    if !is_edit_key && !is_char {
        return false;
    }

    let key = make_key(keysym, modifiers, false);
    match engine::key(key) {
        Some(frame) => {
            log::write(&format!(
                "键 {:#x} handled={} preedit={:?} commit={:?} cand={} mode={}",
                keysym, frame.handled, frame.preedit, frame.commit, frame.cand_count, frame.mode
            ));
            // 上屏
            if !frame.commit.is_empty() {
                input::commit(&frame.commit);
            }
            // 模式变化
            if frame.mode_changed {
                statusbar::set_mode(frame.mode);
                log::info(&format!(
                    "模式切换 → {}",
                    if frame.mode == ffi::WOWTYPE_MODE_ENGLISH { "英文" } else { "中文" }
                ));
            }
            // 候选窗
            if frame.cand_count > 0 || !frame.preedit.is_empty() {
                let items: Vec<String> = (0..frame.cand_count)
                    .filter_map(|i| engine::with_session(|s| s.candidate(i)).flatten())
                    .collect();
                candidate::show(&frame.preedit, items, frame.cand_cursor, frame.has_prev, frame.has_next);
            } else {
                candidate::hide();
            }
            if frame.handled {
                true
            } else {
                // 会话放行：自己把字符插回
                match keys::keysym_to_char(keysym, modifiers) {
                    Some(c) => {
                        input::commit(&c.to_string());
                        true
                    }
                    None => false,
                }
            }
        }
        None => false,
    }
}

fn make_key(keysym: u32, modifiers: u32, release: bool) -> ffi::WowtypeKey {
    ffi::WowtypeKey {
        size: std::mem::size_of::<ffi::WowtypeKey>() as u32,
        keysym,
        modifiers,
        release: if release { 1 } else { 0 },
        prev_char: 0,
        action: ffi::WOWTYPE_KEY_ACTION_NONE,
        hints: 0,
        code: 0,
        repeat: 0,
        time_ms: 0,
    }
}

/// 隐藏消息窗口（备用：接收系统消息）。
#[allow(dead_code)]
fn create_message_window() -> Option<HWND> {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let Ok(hinstance) = windows::Win32::System::LibraryLoader::GetModuleHandleW(None) else {
            return;
        };
        let class_name = wide("WowTypeMessageWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(message_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
    let class_name = wide("WowTypeMessageWindow");
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            windows::core::PCWSTR(class_name.as_ptr()),
            windows::core::PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            10,
            10,
            None,
            None,
            None,
            None,
        )
        .ok()
    }
}

unsafe extern "system" fn message_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
