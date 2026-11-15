//! 全局键盘钩子（WH_KEYBOARD_LL）。
//!
//! 低级键盘钩子：回调在安装线程的消息循环里执行，必须尽快返回。

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// 按键处理回调：vk、是否松开 → 返回 true 表示吃掉（应用收不到）。
pub type KeyHandler = fn(u32, bool) -> bool;

static mut HANDLER: Option<KeyHandler> = None;

/// 安装全局键盘钩子（必须在有消息循环的线程调用）。
pub fn install(handler: KeyHandler) -> bool {
    unsafe {
        HANDLER = Some(handler);
        match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
            Ok(_hook) => {
                // 钩子句柄不保存也行：进程存活期间一直有效
                crate::log::info("全局键盘钩子已安装");
                true
            }
            Err(e) => {
                crate::log::info(&format!("键盘钩子安装失败: {e:?}"));
                false
            }
        }
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam.0 != 0 {
        let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        // 忽略自己 SendInput 注入的按键（LLKHF_INJECTED = 0x10）
        if kb.flags.0 & 0x10 == 0 {
            let msg = wparam.0 as u32;
            let is_up = matches!(msg, x if x == WM_KEYUP || x == WM_SYSKEYUP);
            let is_down = matches!(msg, x if x == WM_KEYDOWN || x == WM_SYSKEYDOWN);
            if is_up || is_down {
                if let Some(handler) = unsafe { HANDLER } {
                    let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        handler(kb.vkCode, is_up)
                    }))
                    .unwrap_or(false);
                    if handled {
                        return LRESULT(1);
                    }
                }
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// 卸载钩子（退出时）。
pub fn uninstall() {
    // 进程退出时系统自动清理；这里留空（HHOOK 未保存）
}
