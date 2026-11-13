//! 采集管线：备份剪贴板 → 模拟 Cmd/Ctrl+C → 读取新文本 → 恢复剪贴板 →
//! 校验 → 追加写入 Obsidian Markdown。全部在本进程内由 Rust + OS API 完成，
//! 不依赖浏览器选区、不打开任何窗口。

use std::time::Duration;

use arboard::{Clipboard, ImageData};

use crate::config::Config;
use crate::markdown;
use crate::platform;

/// 剪贴板备份内容（文本或图片），用于采集后安全恢复。
enum Backup {
    Text(String),
    Image(ImageData<'static>),
}

#[derive(Debug)]
pub struct CaptureOutcome {
    pub word: String,
    /// false = 开启了去重且该单词已存在于目标文件。
    pub appended: bool,
}

/// 执行一次完整采集（真实路径：OS 原生键盘模拟触发复制）。
pub fn capture(cfg: &Config) -> Result<CaptureOutcome, String> {
    capture_with_copy_hook(cfg, platform::simulate_copy)
}

/// 可注入“复制动作”的采集管线。生产路径传 `platform::simulate_copy`；
/// 集成测试注入等效的真实剪贴板写入，以便在无辅助功能权限的环境下
/// 验证备份/读取/恢复/校验/写入全链路。
pub fn capture_with_copy_hook(
    cfg: &Config,
    copy_hook: impl FnOnce() -> Result<(), String>,
) -> Result<CaptureOutcome, String> {
    let path = cfg
        .target_abs_path()
        .ok_or("尚未设置 Obsidian Vault 目录，请先在设置界面选择 Vault")?;

    let mut clip = Clipboard::new().map_err(|e| format!("无法访问系统剪贴板: {e}"))?;

    // 1. 备份当前剪贴板（文本或图片内容都保留）。
    let backup: Option<Backup> = if let Ok(text) = clip.get().text() {
        Some(Backup::Text(text))
    } else {
        clip.get().image().ok().map(Backup::Image)
    };

    // 2. 用操作系统原生键盘模拟触发“复制”。
    copy_hook()?;

    // 3. 读取新复制的文本（部分应用响应复制有延迟，做带重试的轮询）。
    let mut new_text: Option<String> = None;
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(180));
        match clip.get_text() {
            Ok(text) => {
                let is_unchanged = matches!(
                    &backup,
                    Some(Backup::Text(prev)) if *prev == text
                );
                if std::env::var("WORD_COLLECTOR_DIAG").is_ok() {
                    eprintln!("[diag] poll text={:?} unchanged={}", text, is_unchanged);
                }
                if !is_unchanged {
                    new_text = Some(text);
                    break;
                }
            }
            Err(err) => {
                // 剪贴板为空/内容非文本（如截图）时也会走这里；
                // 被其他进程短暂占用则靠重试缓解。
                if std::env::var("WORD_COLLECTOR_DIAG").is_ok() {
                    eprintln!("[diag] poll error: {err}");
                }
            }
        }
    }

    // 4. 安全恢复原剪贴板（无论后续成功与否都先恢复）。
    let restore_result = match backup {
        Some(Backup::Text(text)) => clip.set_text(&text),
        Some(Backup::Image(image)) => clip.set_image(image),
        None => clip.clear(),
    };
    if let Err(err) = restore_result {
        eprintln!("[word-collector] 恢复原剪贴板失败: {err}");
    }

    // 5. 校验文本是否为有效的英文单词/短语。
    let word = validate(new_text.ok_or("没有检测到新的复制内容：请先在应用中选中单词")?.as_str())?;

    // 6. 交给文件写入模块。
    let appended = markdown::append_word(&path, &word, cfg.dedup_enabled)
        .map_err(|e| format!("写入 Markdown 失败（{}）: {e}", path.display()))?;

    Ok(CaptureOutcome { word, appended })
}

/// 校验选中文本：非空、长度合理、无换行、仅英文单词/短语。
fn validate(text: &str) -> Result<String, String> {
    let word = text.trim();
    if word.is_empty() {
        return Err("剪贴板中的文本为空".into());
    }
    if word.len() > 64 {
        return Err("选中文本过长，仅支持单词或短语".into());
    }
    if word.contains(['\n', '\r', '\t']) {
        return Err("选中文本包含换行，仅支持单词或短语".into());
    }
    let valid_chars = word
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '\'' | '\u{2019}'));
    if !valid_chars {
        return Err("仅支持英文单词或短语".into());
    }
    if !word.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err("选中文本中不包含英文字母".into());
    }
    Ok(word.to_string())
}

#[cfg(test)]
mod tests {
    use super::validate;

    #[test]
    fn accepts_words_and_phrases() {
        assert_eq!(validate("  mitigate ").unwrap(), "mitigate");
        assert_eq!(validate("state of the art").unwrap(), "state of the art");
        assert_eq!(validate("don’t").unwrap(), "don’t");
    }

    #[test]
    fn rejects_invalid_selections() {
        assert!(validate("").is_err());
        assert!(validate("   ").is_err());
        assert!(validate("hello\nworld").is_err());
        assert!(validate("中文词语").is_err());
        assert!(validate("123456").is_err());
        assert!(validate(&"a".repeat(65)).is_err());
    }
}
