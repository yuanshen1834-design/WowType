//! DllRegisterServer / DllUnregisterServer 的注册逻辑。
//!
//! 注册三件事（照 TSF 规范）：
//! 1. HKCR\CLSID\{CLSID}\InprocServer32 → DLL 路径 + ThreadingModel=Apartment
//! 2. ITfInputProcessorProfiles::Register + AddLanguageProfile（语言栏可见）
//! 3. ITfCategoryMgr::RegisterCategory ×N（声明能力，缺了 Win10/11 输入切换器会过滤掉）

use crate::{log, CLSID_WOWTYPE, CLSID_WOWTYPE_STR, GUID_PROFILE, LANGID_ZH_CN, SERVICE_DESCRIPTION};
use windows::core::{Result, GUID, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, ITfCategoryMgr,
    ITfInputProcessorProfiles,
};
use windows_registry::CLASSES_ROOT;

fn wide_with_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// COM 环境包装：regsvr32 一般已初始化过，已是别的套间模式就不配对反初始化。
fn com_scope<T>(body: impl FnOnce() -> Result<T>) -> Result<T> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let result = body();
    if initialized {
        unsafe { CoUninitialize() };
    }
    result
}

/// 注册（DllRegisterServer 调用）。
pub fn register() -> Result<()> {
    let module = crate::module_path()?;
    let module_str = module.to_string();
    log::write(&format!("注册 DLL: {module_str}"));

    // 1. CLSID → InprocServer32
    let clsid_key = format!("CLSID\\{CLSID_WOWTYPE_STR}");
    let base = CLASSES_ROOT.create(&clsid_key)?;
    base.set_string("", SERVICE_DESCRIPTION)?;
    let inproc = CLASSES_ROOT.create(&format!("{clsid_key}\\InprocServer32"))?;
    inproc.set_string("", &module_str)?;
    inproc.set_string("ThreadingModel", "Apartment")?;
    log::write("InprocServer32 写入完成");

    // 2. TSF 语言配置注册
    com_scope(|| {
        let profiles: ITfInputProcessorProfiles = unsafe {
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?
        };
        let description = wide_with_nul(SERVICE_DESCRIPTION);
        let icon = wide_with_nul(&module_str);        unsafe {
            profiles.Register(&CLSID_WOWTYPE)?;
            profiles.AddLanguageProfile(
                &CLSID_WOWTYPE,
                LANGID_ZH_CN,
                &GUID_PROFILE,
                &description,
                &icon,
                0,
            )?;
        }
        log::write("语言配置注册完成");

        // 3. 类别（能力声明）
        let category: ITfCategoryMgr =
            unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
        for cat in categories() {
            unsafe { category.RegisterCategory(&CLSID_WOWTYPE, cat, &CLSID_WOWTYPE) }?;
        }
        log::write(&format!("{} 个类别注册完成", categories().len()));
        Ok(())
    })
}

/// 反注册（DllUnregisterServer 调用），严格逆序。
pub fn unregister() -> Result<()> {
    com_scope(|| {
        if let Ok(category) = unsafe {
            CoCreateInstance::<_, ITfCategoryMgr>(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)
        } {
            for cat in categories() {
                let _ = unsafe { category.UnregisterCategory(&CLSID_WOWTYPE, cat, &CLSID_WOWTYPE) };
            }
        }
        if let Ok(profiles) = unsafe {
            CoCreateInstance::<_, ITfInputProcessorProfiles>(
                &CLSID_TF_InputProcessorProfiles,
                None,
                CLSCTX_INPROC_SERVER,
            )
        } {
            let _ = unsafe {
                profiles.RemoveLanguageProfile(&CLSID_WOWTYPE, LANGID_ZH_CN, &GUID_PROFILE)
            };
            let _ = unsafe { profiles.Unregister(&CLSID_WOWTYPE) };
        }
        Ok(())
    })?;

    let clsid_key = format!("CLSID\\{CLSID_WOWTYPE_STR}");
    let _ = CLASSES_ROOT.remove_tree(&clsid_key);
    log::write("反注册完成");
    Ok(())
}

/// 声明的能力类别集合。
fn categories() -> &'static [GUID] {
    use windows::Win32::UI::TextServices::{
        GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_COMLESS, GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
        GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT, GUID_TFCAT_TIPCAP_SECUREMODE,
        GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT, GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
    };
    &[
        GUID_TFCAT_TIP_KEYBOARD,
        GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
        GUID_TFCAT_TIPCAP_SECUREMODE,
        GUID_TFCAT_TIPCAP_COMLESS,
        GUID_TFCAT_TIPCAP_INPUTMODECOMPARTMENT,
        GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
        GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    ]
}
