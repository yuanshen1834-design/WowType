//! TSF 文档编辑（v13 稳定版：无内联组句模式）。
//!
//! 设计（对应代码审查严重 1 的修复）：
//! - 不做内联 composition（拼音串不显示在输入框里，显示在候选窗）
//! - 上屏 = InsertTextAtSelection 直接插入 + 移光标
//! - 因此不再有任何跨线程的组句状态（原先 ITfComposition 存在 thread_local，
//!   而异步 edit session 在 worker 线程执行 → 跨线程断裂）
//! - 每个 edit session 自包含（context + commit 都在结构体里）

use windows::core::{implement, Interface, Result};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfEditSession, ITfEditSession_Impl, ITfInsertAtSelection, ITfRange, TF_AE_END,
    TF_ANCHOR_END, TF_ES_READWRITE, TF_SELECTION, TF_SELECTIONSTYLE,
};

use crate::log;

/// 一次待执行的上屏。
#[implement(ITfEditSession)]
struct UpdateSession {
    context: ITfContext,
    commit: String,
}

impl ITfEditSession_Impl for UpdateSession_Impl {
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        // 从框架的 C++ 调进来：panic 不能越过 FFI。
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            apply(&self.context, ec, &self.commit)
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                log::info(&format!("上屏失败: {e:?}"));
                Err(e)
            }
            Err(_) => {
                log::info("上屏会话 panic（已兜住）");
                Err(windows::Win32::Foundation::E_FAIL.into())
            }
        }
    }
}

fn apply(context: &ITfContext, ec: u32, commit: &str) -> Result<()> {
    if commit.is_empty() {
        return Ok(());
    }
    let utf16: Vec<u16> = commit.encode_utf16().collect();
    let insert: ITfInsertAtSelection = context.cast()?;
    // 标志用 0（不是 NOQUERY：NOQUERY 不回传 range，windows-rs 会当成失败）。
    let range = unsafe {
        insert.InsertTextAtSelection(
            ec,
            windows::Win32::UI::TextServices::INSERT_TEXT_AT_SELECTION_FLAGS(0),
            &utf16,
        )?
    };
    move_selection_to_end(context, ec, &range)?;
    Ok(())
}

/// 请求上屏（异步 edit session，自包含无共享状态）。
pub fn commit_text(context: &ITfContext, client_id: u32, text: String) -> Result<()> {
    if text.is_empty() {
        return Ok(());
    }
    let session: ITfEditSession = UpdateSession {
        context: context.clone(),
        commit: text,
    }
    .into();
    unsafe { context.RequestEditSession(client_id, &session, TF_ES_READWRITE) }?.ok()?;
    Ok(())
}

/// 把选区移到 range 末尾（否则下一个字插到前面，顺序反）。
fn move_selection_to_end(context: &ITfContext, ec: u32, range: &ITfRange) -> Result<()> {
    let end = unsafe { range.Clone()? };
    unsafe { end.Collapse(ec, TF_ANCHOR_END)? };
    let selection = TF_SELECTION {
        range: std::mem::ManuallyDrop::new(Some(end)),
        style: TF_SELECTIONSTYLE {
            ase: TF_AE_END,
            fInterimChar: false.into(),
        },
    };
    let result = unsafe { context.SetSelection(ec, std::slice::from_ref(&selection)) };
    drop(std::mem::ManuallyDrop::into_inner(selection.range));
    result
}
