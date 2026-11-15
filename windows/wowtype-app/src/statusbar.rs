//! 状态条（仿搜狗）：悬浮小窗，显示中/英状态，可拖动、点击切换、右键菜单。

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetDC,
    GetTextExtentPoint32W, ReleaseDC, SelectObject, SetBkMode, SetTextColor, RoundRect, TextOutW,
    CLEARTYPE_QUALITY, DEFAULT_CHARSET, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FW_NORMAL,
    HGDIOBJ, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, GetClientRect,
    GetWindowLongPtrW, IsWindow, LoadCursorW, RegisterClassW, SetCursor, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TrackPopupMenu, GUITHREADINFO, GWLP_USERDATA, HTBORDER, HTCAPTION,
    HTCLIENT, IDC_ARROW, MF_SEPARATOR, MF_STRING, SWP_NOMOVE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SW_HIDE, SW_SHOWNOACTIVATE, TPM_LEFTALIGN, TPM_RIGHTBUTTON, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_NCHITTEST, WM_PAINT, WM_RBUTTONUP, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const CLASS_NAME: &str = "WowTypeStatusBar";
const BAR_WIDTH: i32 = 168;
const BAR_HEIGHT: i32 = 34;
const BTN_TOGGLE: i32 = 3; // 中/英切换按钮区域（left..left+40）

static mut BAR_HWND: HWND = HWND(std::ptr::null_mut());

struct BarState {
    mode: i32, // 0 中文 1 英文
    enabled: bool,
    dragging: bool,
    drag_start: POINT,
    bar_start: (i32, i32),
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn state(hwnd: HWND) -> *mut BarState {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut BarState }
}

fn ensure_class() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let Ok(hinstance) = windows::Win32::System::LibraryLoader::GetModuleHandleW(None) else {
            return;
        };
        let class_name = wide(CLASS_NAME);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
}

pub fn create() -> Option<HWND> {
    unsafe {
        if !BAR_HWND.is_invalid() && IsWindow(Some(BAR_HWND)).as_bool() {
            let _ = ShowWindow(BAR_HWND, SW_SHOWNOACTIVATE);
            return Some(BAR_HWND);
        }
    }
    ensure_class();
    let class_name = wide(CLASS_NAME);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            20,
            20,
            BAR_WIDTH,
            BAR_HEIGHT,
            None,
            None,
            None,
            None,
        )
        .ok()?
    };
    let state = Box::into_raw(Box::new(BarState {
        mode: 0,
        enabled: true,
        dragging: false,
        drag_start: POINT::default(),
        bar_start: (20, 20),
    }));
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state as isize);
        BAR_HWND = hwnd;
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
    Some(hwnd)
}

pub fn set_mode(mode: i32) {
    unsafe {
        if !BAR_HWND.is_invalid() && IsWindow(Some(BAR_HWND)).as_bool() {
            let state = state(BAR_HWND);
            if !state.is_null() {
                (*state).mode = mode;
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(BAR_HWND), None, true);
        }
    }
}

pub fn set_enabled(enabled: bool) {
    unsafe {
        if !BAR_HWND.is_invalid() && IsWindow(Some(BAR_HWND)).as_bool() {
            let state = state(BAR_HWND);
            if !state.is_null() {
                (*state).enabled = enabled;
            }
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(BAR_HWND), None, true);
        }
    }
}

pub fn hide() {
    unsafe {
        if !BAR_HWND.is_invalid() && IsWindow(Some(BAR_HWND)).as_bool() {
            let _ = ShowWindow(BAR_HWND, SW_HIDE);
        }
    }
}

fn paint(hwnd: HWND) {
    let state = state(hwnd);
    if state.is_null() {
        return;
    }
    let hdc = unsafe { GetDC(Some(hwnd)) };
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    // 背景（浅蓝白）
    let bg = unsafe { CreateSolidBrush(COLORREF(0x00FFF5EE)) };
    unsafe {
        FillRect(hdc, &client, bg);
        let _ = DeleteObject(HGDIOBJ(bg.0));
    }
    // 圆角边框
    let pen = unsafe { windows::Win32::Graphics::Gdi::CreatePen(PS_SOLID, 1, COLORREF(0x00D0C0B0)) };
    let old_pen = unsafe { SelectObject(hdc, HGDIOBJ(pen.0)) };
    let bg2 = unsafe { windows::Win32::Graphics::Gdi::GetStockObject(
        windows::Win32::Graphics::Gdi::NULL_BRUSH,
    ) };
    let old_brush = unsafe { SelectObject(hdc, bg2) };
    unsafe {
        let _ = RoundRect(hdc, 0, 0, client.right, client.bottom, 8, 8);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }

    let font = unsafe {
        CreateFontW(
            15,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            FONT_OUTPUT_PRECISION(0),
            FONT_CLIP_PRECISION(0),
            CLEARTYPE_QUALITY,
            0,
            PCWSTR(wide("Microsoft YaHei").as_ptr()),
        )
    };
    let old_font = unsafe { SelectObject(hdc, HGDIOBJ(font.0)) };
    unsafe {
        SetBkMode(hdc, TRANSPARENT);
    }

    // 中/英 按钮
    let mode = unsafe { (*state).mode };
    let (label, color) = if mode == 0 {
        ("中", COLORREF(0x00D06020))
    } else {
        ("英", COLORREF(0x002070C0))
    };
    unsafe {
        if mode == 0 {
            let hl = CreateSolidBrush(COLORREF(0x00FFF0E0));
            let rc = RECT {
                left: 2,
                top: 2,
                right: 46,
                bottom: client.bottom - 2,
            };
            FillRect(hdc, &rc, hl);
            let _ = DeleteObject(HGDIOBJ(hl.0));
        }
        SetTextColor(hdc, color);
        let utf16: Vec<u16> = label.encode_utf16().collect();
        let _ = TextOutW(hdc, 16, 8, &utf16);
    }

    // 标题（暂停时显示灰色 + 停）
    let enabled = unsafe { (*state).enabled };
    unsafe {
        SetTextColor(hdc, if enabled { COLORREF(0x00404040) } else { COLORREF(0x00A0A0A0) });
        let title = if enabled { "WowType" } else { "停用" };
        let utf16: Vec<u16> = title.encode_utf16().collect();
        let _ = TextOutW(hdc, 52, 8, &utf16);
    }

    // 右侧：标点 + 菜单提示
    unsafe {
        SetTextColor(hdc, COLORREF(0x00909090));
        let utf16: Vec<u16> = "。，".encode_utf16().collect();
        let mut size = windows::Win32::Foundation::SIZE::default();
        let _ = GetTextExtentPoint32W(hdc, &utf16, &mut size);
        let _ = TextOutW(hdc, 108, 8, &utf16);
        let utf16 = "☰".encode_utf16().collect::<Vec<u16>>();
        let _ = TextOutW(hdc, 140, 8, &utf16);
    }

    unsafe {
        SelectObject(hdc, old_font);
        let _ = DeleteObject(HGDIOBJ(font.0));
        ReleaseDC(Some(hwnd), hdc);
    }
}

fn show_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        let _ = AppendMenuW(menu, MF_STRING, 100, PCWSTR(wide("显示候选窗").as_ptr()));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, 200, PCWSTR(wide("退出 WowType").as_ptr()));
        let mut point = POINT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wnd_proc_inner(hwnd, msg, wparam, lparam)
    }));
    match result {
        Ok(r) => r,
        Err(_) => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn wnd_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            paint(hwnd);
            unsafe {
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        WM_LBUTTONDOWN => {
            let state = state(hwnd);
            if state.is_null() {
                return LRESULT(0);
            }
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let mut point = POINT::default();
            unsafe {
                let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
            }
            unsafe {
                (*state).dragging = true;
                (*state).drag_start = point;
                (*state).bar_start = (0, 0);
            }
            let _ = x;
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let state = state(hwnd);
            if state.is_null() {
                return LRESULT(0);
            }
            unsafe {
                if (*state).dragging {
                    let mut point = POINT::default();
                    let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
                    let dx = point.x - (*state).drag_start.x;
                    let dy = point.y - (*state).drag_start.y;
                    if dx != 0 || dy != 0 {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            dx,
                            dy,
                            BAR_WIDTH,
                            BAR_HEIGHT,
                            SWP_NOMOVE | SWP_NOZORDER,
                        );
                    }
                }
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let state = state(hwnd);
            if !state.is_null() {
                unsafe {
                    (*state).dragging = false;
                }
            }
            LRESULT(0)
        }
        WM_RBUTTONUP => {
            show_menu(hwnd);
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            if id == 200 {
                std::process::exit(0);
            }
            LRESULT(0)
        }
        windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY => {
            let state = state(hwnd);
            if !state.is_null() {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let _ = Box::from_raw(state);
                }
            }
            unsafe {
                if BAR_HWND == hwnd {
                    BAR_HWND = HWND(std::ptr::null_mut());
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
