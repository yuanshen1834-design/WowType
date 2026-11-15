//! WowType Windows TSF 文本服务 DLL。
//!
//! 结构（P0 阶段）：
//! - `com`：DLL 入口（DllGetClassObject 等）、CLSID、类厂、文本服务对象
//! - `registry`：DllRegisterServer / DllUnregisterServer 的注册逻辑
//! - `log`：调试日志（写入 %LOCALAPPDATA%\WowType\logs\tsf.log）
//!
//! 后续阶段：
//! - P1：按键 → 引擎会话（wowtype_core.dll 的 FFI）→ preedit + commit
//! - P2：候选窗口（自绘）
//! - P3：引擎进程拆分、安装程序

#![allow(non_snake_case)]

mod candidate;
mod com;
mod engine;
mod ffi;
mod itf;
mod keys;
mod log;
mod registry;

pub(crate) use windows::Win32::Foundation::{E_FAIL, HMODULE, S_FALSE, S_OK};
pub(crate) use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
pub(crate) use windows::core::{BOOL, GUID, HRESULT, HSTRING, Interface, Result};

use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, AtomicPtr, Ordering};

/// WowType 文本服务的 CLSID（固定 GUID，安装时注册到系统）。
pub(crate) const CLSID_WOWTYPE: GUID = GUID::from_u128(0x7d4f2a1c_9c3b_4f8e_a1d2_5b6e8f0c3a74);
pub(crate) const CLSID_WOWTYPE_STR: &str = "{7D4F2A1C-9C3B-4F8E-A1D2-5B6E8F0C3A74}";

/// 语言配置 GUID（每个输入法固定一个）。
pub(crate) const GUID_PROFILE: GUID = GUID::from_u128(0x9b3e5c2a_6f1d_4a8b_b7c4_2d8e9f0a1b34);

/// 简体中文 LANGID。
pub(crate) const LANGID_ZH_CN: u16 = 0x0804;

/// 显示名。
pub(crate) const SERVICE_DESCRIPTION: &str = "WowType 拼音";

/// DLL_PROCESS_ATTACH。
const DLL_PROCESS_ATTACH: u32 = 1;

static DLL_REFERENCES: AtomicIsize = AtomicIsize::new(0);
static DLL_MODULE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

thread_local! {
    /// 当前线程激活的文本服务（TSF 每线程一个）。
    /// 故意用 ManuallyDrop：应用退出时 TSF 不调 Deactivate，此时在线程局部析构里
    /// 往 TSF 身上 Release 会让宿主 fail-fast（Word 退出报错），所以不析构。
    pub(crate) static ACTIVE: std::cell::RefCell<Option<std::mem::ManuallyDrop<windows::core::ComObject<com::TextService>>>> =
        const { std::cell::RefCell::new(None) };
}

pub(crate) fn lock_module() {
    DLL_REFERENCES.fetch_add(1, Ordering::SeqCst);
}

pub(crate) fn unlock_module() {
    DLL_REFERENCES.fetch_sub(1, Ordering::SeqCst);
}

pub(crate) fn dll_instance() -> HMODULE {
    HMODULE(DLL_MODULE.load(Ordering::SeqCst))
}

/// 当前线程号当会话号（TSF 的 client id 各进程相同，会撞号）。
pub(crate) fn session_id() -> u64 {
    u64::from(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() })
}

/// 本 DLL 的完整路径。
pub(crate) fn module_path() -> Result<HSTRING> {
    let module = dll_instance();
    let mut buf = [0u16; 260];
    let len = unsafe { GetModuleFileNameW(Some(module), &mut buf) } as usize;
    if len == 0 || len >= buf.len() {
        return Err(E_FAIL.into());
    }
    Ok(HSTRING::from_wide(&buf[..len]))
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_FAIL;
    }
    if unsafe { *rclsid } != CLSID_WOWTYPE {
        return HRESULT(0x8004_0101u32 as i32); // CLASS_E_CLASSNOTAVAILABLE
    }
    let factory: windows::Win32::System::Com::IClassFactory = com::ClassFactory.into();
    unsafe { factory.query(riid, ppv) }
}

#[unsafe(no_mangle)]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    if DLL_REFERENCES.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllRegisterServer() -> HRESULT {
    log::init();
    log::write("DllRegisterServer 被调用");
    match registry::register() {
        Ok(()) => {
            log::write("注册成功");
            S_OK
        }
        Err(e) => {
            log::write(&format!("注册失败: {e:?}"));
            e.code()
        }
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllUnregisterServer() -> HRESULT {
    log::init();
    log::write("DllUnregisterServer 被调用");
    match registry::unregister() {
        Ok(()) => S_OK,
        Err(e) => {
            log::write(&format!("反注册失败: {e:?}"));
            e.code()
        }
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(hinst: HMODULE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DLL_MODULE.store(hinst.0, Ordering::SeqCst);
    }
    BOOL(1)
}
