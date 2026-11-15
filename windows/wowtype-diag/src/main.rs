//! WowType 输入法诊断/修复工具。
//!
//! 用官方 TSF API（ITfInputProcessorProfiles）：
//! 1. 枚举语言配置文件，检查 WowType 是否在 Win+Space 列表
//! 2. 检查/设置 Enable 状态
//! 3. 不在列表 → AddLanguageProfile 添加（官方注册，保证 Win+Space 可见）
//! 4. 可选：激活 WowType（ActivateLanguageProfile）

#![windows_subsystem = "console"]

use windows::core::{GUID, Interface, Result, BOOL};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::TextServices::{
    ITfInputProcessorProfiles, IEnumTfLanguageProfiles, TF_LANGUAGEPROFILE,
};

const CLSID_TF_IPP: GUID = GUID::from_u128(0x33C53A50_F456_4884_B049_85FD643ECFED);
const WOWTYPE_CLSID: GUID = GUID::from_u128(0x7D4F2A1C_9C3B_4F8E_A1D2_5B6E8F0C3A74);
const GUID_PROFILE: GUID = GUID::from_u128(0x9B3E5C2A_6F1D_4A8B_B7C4_2D8E9F0A1B34);
const GUID_TFCAT_TIP_KEYBOARD: GUID = GUID::from_u128(0x34745C63_B2F0_4784_8B67_5E12C8701A31);
const LANG_ZH: u16 = 0x0804;

fn main() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let result = run();
    println!();
    match result {
        Ok(summary) => println!("✅ {summary}"),
        Err(e) => println!("❌ 失败: {e}"),
    }
    println!();
    println!("按回车退出...");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
}

fn run() -> Result<String> {
    let ipp: ITfInputProcessorProfiles =
        unsafe { CoCreateInstance(&CLSID_TF_IPP, None, CLSCTX_INPROC_SERVER)? };

    println!("=== 1. 当前语言配置文件列表 ===");
    let mut found = false;
    let mut enabled = false;
    let mut active = false;

    for lang in [0x0804u16, 0xFFFF] {
        let profiles = enum_profiles(&ipp, lang)?;
        for p in profiles {
            let is_wow = p.clsid == WOWTYPE_CLSID;
            if is_wow {
                found = true;
                let en = unsafe {
                    ipp.IsEnabledLanguageProfile(&WOWTYPE_CLSID, lang, &p.guidProfile)
                };
                enabled = en.is_ok() && en.unwrap().as_bool();
                active = p.fActive.as_bool();
            }
            println!(
                "  Lang={lang:04X} CLSID={:?} Active={} Profile={:?}{}",
                p.clsid,
                p.fActive.as_bool(),
                p.guidProfile,
                if is_wow { " ← ★ WowType" } else { "" }
            );
        }
    }

    if !found {
        println!();
        println!("=== 2. WowType 不在列表 → 用官方 API 注册 ===");
        // AddLanguageProfile(clsid, langid, guidProfile, description, iconfile, iconindex)
        let desc: Vec<u16> = "WowType Pinyin"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            ipp.AddLanguageProfile(&WOWTYPE_CLSID, LANG_ZH, &GUID_PROFILE, &desc, &[], 0)
        };
        match result {
            Ok(()) => println!("✅ AddLanguageProfile(0x0804) 成功"),
            Err(e) => println!("AddLanguageProfile(0x0804) 失败: {e}（可能已存在）"),
        }
        println!("=== 3. 设置为启用 ===");
        let result = unsafe { ipp.EnableLanguageProfile(&WOWTYPE_CLSID, LANG_ZH, &GUID_PROFILE, true) };
        match result {
            Ok(()) => println!("✅ EnableLanguageProfile 成功"),
            Err(e) => println!("EnableLanguageProfile 失败: {e}"),
        }
        println!("=== 4. 激活 WowType（当前线程切换）===");
        let result =
            unsafe { ipp.ActivateLanguageProfile(&WOWTYPE_CLSID, LANG_ZH, &GUID_PROFILE) };
        match result {
            Ok(()) => println!("✅ 已激活 WowType（当前会话）"),
            Err(e) => println!("激活失败: {e}"),
        }
        return Ok("WowType 已注册到系统输入法列表，重启应用后 Win+Space 可见".into());
    }

    println!();
    println!("=== WowType 状态 ===");
    println!("  在语言配置文件列表: ✅");
    println!("  Enable: {}", if enabled { "✅ 已启用" } else { "❌ 未启用" });
    println!("  当前激活: {}", if active { "✅" } else { "否" });

    if !enabled {
        println!();
        println!("=== 修复：启用语言配置文件 ===");
        for lang in [0x0804u16, 0xFFFF] {
            let r = unsafe { ipp.EnableLanguageProfile(&WOWTYPE_CLSID, lang, &GUID_PROFILE, true) };
            println!("  Enable({lang:04X}): {}", if r.is_ok() { "✅" } else { "失败/已启用" });
        }
    }
    if !active {
        println!();
        println!("=== 激活 WowType（当前线程）===");
        let r = unsafe { ipp.ActivateLanguageProfile(&WOWTYPE_CLSID, LANG_ZH, &GUID_PROFILE) };
        println!("  激活: {}", if r.is_ok() { "✅" } else { "失败" });
    }
    Ok("WowType 输入法注册状态正常".into())
}

fn enum_profiles(ipp: &ITfInputProcessorProfiles, lang: u16) -> Result<Vec<TF_LANGUAGEPROFILE>> {
    let e: IEnumTfLanguageProfiles = unsafe { ipp.EnumLanguageProfiles(lang)? };
    let mut out = Vec::new();
    loop {
        let mut fetched = 0u32;
        let mut buf = [TF_LANGUAGEPROFILE::default(); 16];
        let hr = unsafe { e.Next(&mut buf, &mut fetched) };
        if hr.is_ok() {
            for i in 0..fetched as usize {
                out.push(buf[i]);
            }
            if fetched == 0 {
                break;
            }
        } else {
            break;
        }
    }
    Ok(out)
}
