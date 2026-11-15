//! 候选窗口（独立 App 版）：仿搜狗
//! - 第一行显示拼音（编辑区）
//! - 第二行显示候选（数字序号、当前项高亮、翻页提示）
//! - 跟随光标定位（GetGUIThreadInfo，失败退回鼠标）
//!
//! 线程模型：exe 单线程（消息循环），所有绘制在主线程完成。

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, EndPaint, FillRect, GetDC,
    GetTextExtentPoint32W, Rectangle, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TextOutW,
    CLEARTYPE_QUALITY, DEFAULT_CHARSET, FONT_CLIP_PRECISION, FONT_OUTPUT_PRECISION, FW_NORMAL,
    HGDIOBJ, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetForegroundWindow, GetGUIThreadInfo,
    GetWindowLongPtrW, IsWindow, IsWindowVisible, RegisterClassW, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, GUITHREADINFO, GWLP_USERDATA, HWND_TOPMOST, MA_NOACTIVATE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_HIDE, SW_SHOWNOACTIVATE, WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const CLASS_NAME: &str = "WowTypeCandidateApp";
const PADDING_X: i32 = 12;
const PADDING_Y: i32 = 6;
const ITEM_GAP: i32 = 10;
const FONT_SIZE: i32 = 22;
const ITEM_HEIGHT: i32 = 28;
const PREEDIT_HEIGHT: i32 = 26;

static mut HWND_STATE: HWND = HWND(std::ptr::null_mut());

struct CandidateData {
    preedit: String,
    items: Vec<String>,
    cursor: i32,
    has_prev: bool,
    has_next: bool,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn window_data(hwnd: HWND) -> *mut CandidateData {
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut CandidateData }
}

fn create_font(size: i32) -> windows::Win32::Graphics::Gdi::HFONT {
    let face = wide("Microsoft YaHei");
    unsafe {
        CreateFontW(
            size,
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
            PCWSTR(face.as_ptr()),
        )
    }
}

fn text_width(hdc: windows::Win32::Graphics::Gdi::HDC, text: &str, font_size: i32) -> i32 {
    let font = create_font(font_size);
    let old = unsafe { SelectObject(hdc, HGDIOBJ(font.0)) };
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    unsafe {
        let _ = GetTextExtentPoint32W(hdc, &utf16, &mut size);
        SelectObject(hdc, old);
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
    size.cx
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
            ..Default::default()
        };
        RegisterClassW(&wc);
    });
}

fn ensure_window() -> Option<HWND> {
    unsafe {
        if !HWND_STATE.is_invalid() && IsWindow(Some(HWND_STATE)).as_bool() {
            return Some(HWND_STATE);
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
            0,
            0,
            10,
            10,
            None,
            None,
            None,
            None,
        )
        .ok()?
    };
    let data = Box::into_raw(Box::new(CandidateData {
        preedit: String::new(),
        items: Vec::new(),
        cursor: -1,
        has_prev: false,
        has_next: false,
    }));
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);
        HWND_STATE = hwnd;
    }
    Some(hwnd)
}

/// 光标位置（前台线程 caret，失败退回鼠标）。
fn caret_position() -> (i32, i32) {
    unsafe {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(0, &mut info).is_ok() && !info.hwndCaret.is_invalid() {
            let mut point = POINT {
                x: info.rcCaret.left,
                y: info.rcCaret.bottom + 2,
            };
            if windows::Win32::Graphics::Gdi::ClientToScreen(info.hwndCaret, &mut point).as_bool() {
                return (point.x, point.y);
            }
        }
        let mut point = POINT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
        let _ = GetForegroundWindow();
        (point.x, point.y + 24)
    }
}

/// 显示候选窗（拼音 + 候选）。空则隐藏。
pub fn show(preedit: &str, items: Vec<String>, cursor: i32, has_prev: bool, has_next: bool) {
    if items.is_empty() && preedit.is_empty() {
        hide();
        return;
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        show_inner(preedit, items, cursor, has_prev, has_next)
    }));
    if result.is_err() {
        crate::log::info("候选窗口 show panic（已兜住）");
    }
}

fn show_inner(preedit: &str, items: Vec<String>, cursor: i32, has_prev: bool, has_next: bool) {
    let Some(hwnd) = ensure_window() else {
        return;
    };
    let data = window_data(hwnd);
    if data.is_null() {
        return;
    }
    unsafe {
        (*data).preedit = preedit.to_string();
        (*data).items = items;
        (*data).cursor = cursor;
        (*data).has_prev = has_prev;
        (*data).has_next = has_next;
    }
    let (x, y) = caret_position();
    layout_and_show(hwnd, x, y);
}

pub fn hide() {
    unsafe {
        if !HWND_STATE.is_invalid() && IsWindow(Some(HWND_STATE)).as_bool() {
            if IsWindowVisible(HWND_STATE).as_bool() {
                let _ = ShowWindow(HWND_STATE, SW_HIDE);
            }
        }
    }
}

pub fn visible() -> bool {
    unsafe {
        !HWND_STATE.is_invalid() && IsWindow(Some(HWND_STATE)).as_bool() && IsWindowVisible(HWND_STATE).as_bool()
    }
}

fn layout_and_show(hwnd: HWND, x: i32, y: i32) {
    let data = window_data(hwnd);
    if data.is_null() {
        return;
    }
    let hdc = unsafe { GetDC(None) };
    let preedit = unsafe { &(*data).preedit };
    let items = unsafe { &(*data).items };
    let has_prev = unsafe { (*data).has_prev };
    let has_next = unsafe { (*data).has_next };

    let mut width = 100;
    if !preedit.is_empty() {
        width = width.max(text_width(hdc, preedit, FONT_SIZE) + PADDING_X * 2);
    }
    let mut total = PADDING_X * 2;
    for (index, item) in items.iter().enumerate() {
        let label = format!("{}{}", index + 1, item);
        total += text_width(hdc, &label, FONT_SIZE) + ITEM_GAP;
    }
    if !items.is_empty() {
        total -= ITEM_GAP;
    }
    width = width.max(total);
    unsafe { ReleaseDC(None, hdc) };

    let mut height = PADDING_Y * 2;
    if !preedit.is_empty() {
        height += PREEDIT_HEIGHT;
    }
    if !items.is_empty() {
        height += ITEM_HEIGHT;
    }
    if has_prev || has_next {
        height += 18;
    }

    // 屏幕内夹紧
    let screen = unsafe {
        let mut rect = RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
            windows::Win32::UI::WindowsAndMessaging::SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut RECT as *mut std::ffi::c_void),
            windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        rect
    };
    let x = x.clamp(screen.left, (screen.right - width).max(screen.left));
    let y = y.clamp(screen.top, (screen.bottom - height).max(screen.top));

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
        let _ = windows::Win32::Graphics::Gdi::UpdateWindow(hwnd);
    }
}

fn paint(hwnd: HWND) {
    let data = window_data(hwnd);
    if data.is_null() {
        return;
    }
    let hdc = unsafe { GetDC(Some(hwnd)) };
    let mut client = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client);
    }
    // 白底
    let white = unsafe { CreateSolidBrush(COLORREF(0x00FFFFFF)) };
    unsafe {
        FillRect(hdc, &client, white);
        let _ = DeleteObject(HGDIOBJ(white.0));
    }
    // 边框
    let pen = unsafe { CreatePen(PS_SOLID, 1, COLORREF(0x00C0B0A0)) };
    let old_pen = unsafe { SelectObject(hdc, HGDIOBJ(pen.0)) };
    unsafe {
        let _ = Rectangle(hdc, client.left, client.top, client.right, client.bottom);
        SelectObject(hdc, old_pen);
        let _ = DeleteObject(HGDIOBJ(pen.0));
    }

    let data_ref = unsafe { &*data };
    let mut y = PADDING_Y;

    // 第一行：拼音（灰色小字）
    if !data_ref.preedit.is_empty() {
        let font = create_font(18);
        let old_font = unsafe { SelectObject(hdc, HGDIOBJ(font.0)) };
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00707070));
            let utf16: Vec<u16> = data_ref.preedit.encode_utf16().collect();
            let _ = TextOutW(hdc, PADDING_X, y, &utf16);
            SelectObject(hdc, old_font);
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
        y += PREEDIT_HEIGHT;
    }

    // 第二行：候选
    if !data_ref.items.is_empty() {
        let font = create_font(FONT_SIZE);
        let old_font = unsafe { SelectObject(hdc, HGDIOBJ(font.0)) };
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
        }
        let mut x = PADDING_X;
        for (index, item) in data_ref.items.iter().enumerate() {
            let label = format!("{}{}", index + 1, item);
            let w = text_width(hdc, &label, FONT_SIZE);
            if index as i32 == data_ref.cursor {
                let highlight = unsafe { CreateSolidBrush(COLORREF(0x00FFE8D0)) };
                let rc = RECT {
                    left: x - 4,
                    top: y - 2,
                    right: x + w + 4,
                    bottom: y + ITEM_HEIGHT,
                };
                unsafe {
                    FillRect(hdc, &rc, highlight);
                    let _ = DeleteObject(HGDIOBJ(highlight.0));
                }
            }
            unsafe {
                SetTextColor(hdc, COLORREF(0x00202020));
                let utf16: Vec<u16> = label.encode_utf16().collect();
                let _ = TextOutW(hdc, x, y, &utf16);
            }
            x += w + ITEM_GAP;
        }
        unsafe {
            SelectObject(hdc, old_font);
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
        y += ITEM_HEIGHT;
    }

    // 翻页提示
    if data_ref.has_prev || data_ref.has_next {
        let font = create_font(14);
        let old_font = unsafe { SelectObject(hdc, HGDIOBJ(font.0)) };
        unsafe {
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00909090));
            let hint = format!(
                "{}  翻页: - =",
                if data_ref.has_prev && data_ref.has_next {
                    "◀ ▶"
                } else if data_ref.has_prev {
                    "◀"
                } else {
                    "▶"
                }
            );
            let utf16: Vec<u16> = hint.encode_utf16().collect();
            let _ = TextOutW(hdc, PADDING_X, y, &utf16);
            SelectObject(hdc, old_font);
            let _ = DeleteObject(HGDIOBJ(font.0));
        }
    }

    unsafe { ReleaseDC(Some(hwnd), hdc) };
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
        windows::Win32::UI::WindowsAndMessaging::WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY => {
            let data = window_data(hwnd);
            if !data.is_null() {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    let _ = Box::from_raw(data);
                }
            }
            unsafe {
                if HWND_STATE == hwnd {
                    HWND_STATE = HWND(std::ptr::null_mut());
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
