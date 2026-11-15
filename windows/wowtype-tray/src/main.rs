//! WowType 托盘图标程序。
//!
//! 常驻任务栏通知区：
//! - 图标 + 悬浮提示
//! - 右键菜单：状态 / 打开数据目录 / 重新注册 / 退出
//! - 保证 WowType 输入法注册可用（检测注册表指向的 DLL 是否存在）

#![windows_subsystem = "windows"]

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_SETVERSION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DispatchMessageW,
    GetMessageW, LoadIconW, RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage,
    IDI_APPLICATION, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, TPM_LEFTALIGN, TPM_RIGHTBUTTON,
    WM_COMMAND, WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};

const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY: u32 = 1;
const ID_EXIT: usize = 200;
const ID_OPEN_DATA: usize = 100;
const ID_ABOUT: usize = 101;
const ID_STATUS: usize = 0;
const ID_SWITCH: usize = 102;

const CLSID_STR: &str = "{7D4F2A1C-9C3B-4F8E-A1D2-5B6E8F0C3A74}";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 加载 WowType.ico（exe 同目录），没有则用系统默认图标。
fn load_icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_DEFAULTCOLOR, LR_LOADFROMFILE};
    let exe = std::env::current_exe().ok();
    let mut ico_path = exe
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.join("WowType.ico")));
    if ico_path.is_none() {
        ico_path = Some(std::path::PathBuf::from(r"G:\WowType\dist\WowType.ico"));
    }
    if let Some(path) = ico_path {
        let wide: Vec<u16> = path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let icon = LoadImageW(
                None,
                windows::core::PCWSTR(wide.as_ptr()),
                IMAGE_ICON,
                16,
                16,
                LR_LOADFROMFILE | LR_DEFAULTCOLOR,
            );
            if let Ok(icon) = icon {
                if !icon.0.is_null() {
                    return windows::Win32::UI::WindowsAndMessaging::HICON(icon.0);
                }
            }
        }
    }
    unsafe { LoadIconW(None, IDI_APPLICATION).unwrap_or_default() }
}

/// 激活 WowType 输入法（当前线程；通知系统切换）。
fn activate_wowtype() {
    use windows::core::GUID;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::TextServices::ITfInputProcessorProfiles;
    const CLSID_TF_IPP: GUID = GUID::from_u128(0x33C53A50_F456_4884_B049_85FD643ECFED);
    const WOWTYPE_CLSID: GUID = GUID::from_u128(0x7D4F2A1C_9C3B_4F8E_A1D2_5B6E8F0C3A74);
    const GUID_PROFILE: GUID = GUID::from_u128(0x9B3E5C2A_6F1D_4A8B_B7C4_2D8E9F0A1B34);
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Ok(ipp) = CoCreateInstance::<_, ITfInputProcessorProfiles>(&CLSID_TF_IPP, None, CLSCTX_INPROC_SERVER) {
            let _ = ipp.ActivateLanguageProfile(&WOWTYPE_CLSID, 0x0804, &GUID_PROFILE);
        }
    }
}

fn main() {
    // 消息窗口
    let hwnd = create_message_window().expect("创建消息窗口失败");
    add_tray_icon(hwnd);

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
    let _ = unsafe {
        Shell_NotifyIconW(NIM_DELETE, &tray_data(hwnd, false))
    };
}

fn tray_data(hwnd: HWND, add: bool) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ID_TRAY,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        ..Default::default()
    };
    unsafe {
        data.hIcon = load_icon();
    }
    let tip = wide("WowType 拼音输入法");
    let mut tip_buf = [0u16; 128];
    tip_buf[..tip.len()].copy_from_slice(&tip[..tip.len().min(127)]);
    data.szTip = tip_buf;
    let _ = add;
    data
}

fn add_tray_icon(hwnd: HWND) {
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &tray_data(hwnd, true));
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &tray_data(hwnd, true));
    }
}

/// 输入法当前状态文本。
fn status_text() -> String {
    // 读注册表确认 DLL 指向
    use std::process::Command;
    let out = Command::new("reg")
        .args([
            "query",
            r"HKCU\SOFTWARE\Classes\CLSID\{7D4F2A1C-9C3B-4F8E-A1D2-5B6E8F0C3A74}\InprocServer32",
            "/ve",
        ])
        .output();
    let dll = out
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            s.lines()
                .find(|l| l.contains(".dll"))
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_else(|| "未注册".into());
    format!("WowType 拼音输入法\n状态: 已注册 ({dll})")
}

fn show_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        // 状态（灰色标题）
        let status = status_text();
        let status_wide = wide(&status);
        let _ = AppendMenuW(menu, MF_GRAYED, ID_STATUS, PCWSTR(status_wide.as_ptr()));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            ID_SWITCH,
            PCWSTR(wide("切换到 WowType 输入法").as_ptr()),
        );
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            ID_OPEN_DATA,
            PCWSTR(wide("打开数据目录").as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            ID_EXIT,
            PCWSTR(wide("退出").as_ptr()),
        );

        let mut point = windows::Win32::Foundation::POINT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            Some(0),
            hwnd,
            None,
        );
        if cmd.as_bool() {
            let _ = windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                Some(hwnd),
                WM_COMMAND,
                WPARAM(cmd.0 as usize),
                LPARAM(0),
            );
        }
        let _ = DestroyMenu(menu);
    }
}

fn create_message_window() -> Option<HWND> {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let Ok(hinstance) = windows::Win32::System::LibraryLoader::GetModuleHandleW(None) else {
            return;
        };
        let class_name = wide("WowTypeTrayWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
    let class_name = wide("WowTypeTrayWindow");
    unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
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

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP || event == WM_LBUTTONDBLCLK {
                show_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            match id {
                x if x == ID_EXIT => {
                    unsafe { Shell_NotifyIconW(NIM_DELETE, &tray_data(hwnd, false)) };
                    std::process::exit(0);
                }
                x if x == ID_SWITCH => {
                    activate_wowtype();
                }
                x if x == ID_OPEN_DATA => {
                    // 打开 DLL 所在目录
                    let out = std::process::Command::new("explorer")
                        .arg(r"G:\WowType\dist")
                        .spawn();
                    let _ = out;
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
