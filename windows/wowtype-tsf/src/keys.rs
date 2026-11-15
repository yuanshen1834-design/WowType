//! 键码：Windows 虚拟键 → XKB keysym（core 会话用）。
//!
//! core 的 keysym 是 XKB keysym 的子集：字母/数字/标点用 ASCII 值，功能键用 0xffXX。

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CAPITAL, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};

use crate::ffi::{
    WOWTYPE_MOD_ALT, WOWTYPE_MOD_CAPS, WOWTYPE_MOD_CTRL, WOWTYPE_MOD_NUMLOCK, WOWTYPE_MOD_SHIFT,
    WOWTYPE_MOD_SUPER,
};

pub const XK_BACKSPACE: u32 = 0xff08;
pub const XK_TAB: u32 = 0xff09;
pub const XK_RETURN: u32 = 0xff0d;
pub const XK_ESCAPE: u32 = 0xff1b;
pub const XK_DELETE: u32 = 0xffff;
pub const XK_HOME: u32 = 0xff50;
pub const XK_LEFT: u32 = 0xff51;
pub const XK_UP: u32 = 0xff52;
pub const XK_RIGHT: u32 = 0xff53;
pub const XK_DOWN: u32 = 0xff54;
pub const XK_PAGE_UP: u32 = 0xff55;
pub const XK_PAGE_DOWN: u32 = 0xff56;
pub const XK_END: u32 = 0xff57;
pub const XK_SHIFT_L: u32 = 0xffe1;
pub const XK_SHIFT_R: u32 = 0xffe2;
pub const XK_CONTROL_L: u32 = 0xffe3;
pub const XK_SPACE: u32 = 0x20;

/// Windows VK → XKB keysym。返回 0 表示不交给会话。
pub fn vk_to_keysym(vk: u32) -> u32 {
    match vk {
        0x41..=0x5A => 0x61 + (vk - 0x41), // 'a'..'z'
        0x30..=0x39 => 0x30 + (vk - 0x30), // '0'..'9'
        0x08 => XK_BACKSPACE,
        0x09 => XK_TAB,
        0x0D => XK_RETURN,
        0x1B => XK_ESCAPE,
        0x20 => XK_SPACE,
        0x2E => XK_DELETE,
        0x24 => XK_HOME,
        0x23 => XK_END,
        0x25 => XK_LEFT,
        0x26 => XK_UP,
        0x27 => XK_RIGHT,
        0x28 => XK_DOWN,
        0x21 => XK_PAGE_UP,
        0x22 => XK_PAGE_DOWN,
        0xA0 => XK_SHIFT_L,
        0xA1 => XK_SHIFT_R,
        0xA2 => XK_CONTROL_L,
        // 标点（VK_OEM_*）
        0xBA => 0x3b, // ;
        0xBB => 0x3d, // =
        0xBC => 0x2c, // ,
        0xBD => 0x2d, // -
        0xBE => 0x2e, // .
        0xBF => 0x2f, // /
        0xC0 => 0x60, // `
        0xDB => 0x5b, // [
        0xDC => 0x5c, // \
        0xDD => 0x5d, // ]
        0xDE => 0x27, // '
        _ => 0,
    }
}

fn key_down(vk: i32) -> bool {
    unsafe { GetKeyState(vk) < 0 }
}

fn key_toggled(vk: i32) -> bool {
    unsafe { GetKeyState(vk) & 1 != 0 }
}

/// 当前修饰键状态（WOWTYPE_MOD_*）。
pub fn current_modifiers() -> u32 {
    let mut mods = 0u32;
    if key_down(VK_SHIFT.0 as i32) {
        mods |= WOWTYPE_MOD_SHIFT;
    }
    if key_down(VK_CONTROL.0 as i32) {
        mods |= WOWTYPE_MOD_CTRL;
    }
    if key_down(VK_MENU.0 as i32) {
        mods |= WOWTYPE_MOD_ALT;
    }
    if key_down(VK_LWIN.0 as i32) || key_down(VK_RWIN.0 as i32) {
        mods |= WOWTYPE_MOD_SUPER;
    }
    if key_toggled(VK_CAPITAL.0 as i32) {
        mods |= WOWTYPE_MOD_CAPS;
    }
    if key_toggled(0x90) {
        // VK_NUMLOCK
        mods |= WOWTYPE_MOD_NUMLOCK;
    }
    mods
}

/// keysym → 可打印字符（按 Shift 状态）。功能键返回 None。
/// 用于会话放行（handled=0）时自己把字符插回文档。
pub fn keysym_to_char(keysym: u32, modifiers: u32) -> Option<char> {
    let shift = modifiers & WOWTYPE_MOD_SHIFT != 0;
    let caps = modifiers & WOWTYPE_MOD_CAPS != 0;
    let upper = shift != caps;
    match keysym {
        0x61..=0x7a => {
            let base = (keysym - 0x61) as u8;
            Some(if upper { (b'A' + base) as char } else { (b'a' + base) as char })
        }
        0x30..=0x39 if !shift => Some((keysym as u8) as char),
        0x30..=0x39 => Some(match keysym {
            0x30 => ')',
            0x31 => '!',
            0x32 => '@',
            0x33 => '#',
            0x34 => '$',
            0x35 => '%',
            0x36 => '^',
            0x37 => '&',
            0x38 => '*',
            _ => '(',
        }),
        0x20 => Some(' '),
        0x2c => Some(if shift { '<' } else { ',' }),
        0x2e => Some(if shift { '>' } else { '.' }),
        0x2f => Some(if shift { '?' } else { '/' }),
        0x3b => Some(if shift { ':' } else { ';' }),
        0x27 => Some(if shift { '"' } else { '\'' }),
        0x5b => Some(if shift { '{' } else { '[' }),
        0x5d => Some(if shift { '}' } else { ']' }),
        0x2d => Some(if shift { '_' } else { '-' }),
        0x3d => Some(if shift { '+' } else { '=' }),
        0x60 => Some(if shift { '~' } else { '`' }),
        0x5c => Some(if shift { '|' } else { '\\' }),
        _ => None,
    }
}
