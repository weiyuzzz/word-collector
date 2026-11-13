//! 端到端集成测试：验证完整采集管线
//! 备份剪贴板 → 触发复制 → 读取新文本 → 恢复剪贴板 → 校验 → 追加写入。
//!
//! 与生产路径的唯一差别：macOS 的 CGEventPost 键盘注入需要用户在系统设置中
//! 手工授予辅助功能权限，无法在 CI/无头环境中完成；因此这里注入
//! “复制动作的等效结果”——用真实 OS 剪贴板写入同一个文本。
//! 其余所有环节（真实剪贴板读写与恢复、校验、真实文件系统写入）全部一致。

use arboard::Clipboard;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use word_collector::collector::capture_with_copy_hook;
use word_collector::config::Config;

/// macOS 的 NSPasteboard 不允许并发剪贴板操作（会崩溃），
/// 测试间用互斥锁串行化所有剪贴板访问。
static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "word-collector-e2e-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn test_config(dir: &PathBuf, dedup: bool) -> Config {
    Config {
        vault_dir: Some(dir.clone()),
        target_file: "vocab/words.md".into(),
        shortcut: "ctrl+alt+w".into(),
        dedup_enabled: dedup,
        autostart: false,
        notifications_enabled: false,
    }
}

fn set_clipboard(text: &str) {
    Clipboard::new().unwrap().set_text(text).unwrap();
}

fn read_clipboard() -> String {
    Clipboard::new().unwrap().get().text().unwrap()
}

fn copy_hook(word: &'static str) -> impl FnOnce() -> Result<(), String> {
    move || {
        // 等效于系统级 Cmd/Ctrl+C 的结果：真实写入 OS 剪贴板
        Clipboard::new()
            .unwrap()
            .set_text(word)
            .map_err(|e| format!("{e}"))
    }
}

#[test]
fn full_pipeline_two_words_one_per_line() {
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = temp_dir("two-words");
    let cfg = test_config(&dir, false);

    // 预置原剪贴板内容（哨兵）
    set_clipboard("SENTINEL");

    let out = capture_with_copy_hook(&cfg, copy_hook("mitigate")).expect("第一次采集应成功");
    assert_eq!(out.word, "mitigate");
    assert!(out.appended);

    let out = capture_with_copy_hook(&cfg, copy_hook("comprehensive")).expect("第二次采集应成功");
    assert_eq!(out.word, "comprehensive");

    let out = capture_with_copy_hook(&cfg, copy_hook("sustainable")).expect("第三次采集应成功");
    assert_eq!(out.word, "sustainable");

    let content = fs::read_to_string(dir.join("vocab/words.md")).unwrap();
    assert_eq!(content, "mitigate\ncomprehensive\nsustainable\n");

    // 采集后原剪贴板被安全恢复
    assert_eq!(read_clipboard(), "SENTINEL");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn dedup_prevents_duplicate_lines_and_clipboard_is_restored() {
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = temp_dir("dedup");
    let cfg = test_config(&dir, true);
    set_clipboard("SENTINEL");

    assert!(capture_with_copy_hook(&cfg, copy_hook("mitigate")).unwrap().appended);
    let again = capture_with_copy_hook(&cfg, copy_hook("mitigate")).unwrap();
    assert!(!again.appended, "重复单词应被去重跳过");

    let content = fs::read_to_string(dir.join("vocab/words.md")).unwrap();
    assert_eq!(content, "mitigate\n");
    assert_eq!(read_clipboard(), "SENTINEL");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_new_copy_reports_error_and_keeps_clipboard_intact() {
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = temp_dir("no-copy");
    let cfg = test_config(&dir, true);
    set_clipboard("SENTINEL");

    // 模拟“什么都没选中”：复制动作没有改变剪贴板
    let err = capture_with_copy_hook(&cfg, || Ok(())).unwrap_err();
    assert!(err.contains("没有检测到新的复制内容"), "实际错误: {err}");

    // 剪贴板未被破坏
    assert_eq!(read_clipboard(), "SENTINEL");
    // 没有写入任何文件
    assert!(!dir.join("vocab/words.md").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn appends_to_file_without_trailing_newline_correctly() {
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let dir = temp_dir("existing-file");
    let cfg = test_config(&dir, false);
    fs::create_dir_all(dir.join("vocab")).unwrap();
    fs::write(dir.join("vocab/words.md"), "已有内容末尾无换行").unwrap();
    set_clipboard("SENTINEL");

    capture_with_copy_hook(&cfg, copy_hook("mitigate")).unwrap();

    let content = fs::read_to_string(dir.join("vocab/words.md")).unwrap();
    assert_eq!(content, "已有内容末尾无换行\nmitigate\n");
    assert_eq!(read_clipboard(), "SENTINEL");

    let _ = fs::remove_dir_all(&dir);
}
