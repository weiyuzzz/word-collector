//! Obsidian Markdown 文件追加写入。
//! 规则：每个单词独占一行；只追加不覆盖；自动补齐文件末尾换行；
//! 可选重复检测（按整行精确匹配）。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

/// 把一个单词追加到目标 Markdown 文件。
/// 返回 Ok(true) 表示已追加一行；Ok(false) 表示开启了去重且单词已存在。
pub fn append_word(path: &Path, word: &str, dedup: bool) -> Result<bool, std::io::Error> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let existing = fs::read_to_string(path).unwrap_or_default();

    if dedup && existing.lines().any(|line| line.trim() == word) {
        return Ok(false);
    }

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    // 自动处理文件末尾换行：已有内容但没有以换行结尾时先补一个换行，
    // 避免出现 "oldwordnewword" 粘连。
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file)?;
    }

    writeln!(file, "{word}")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "word-collector-tests-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn creates_file_and_writes_one_word_per_line() {
        let dir = temp_dir("create");
        let path = dir.join("words.md");
        for word in ["mitigate", "comprehensive", "sustainable"] {
            assert!(append_word(&path, word, false).unwrap());
        }
        assert_eq!(read(&path), "mitigate\ncomprehensive\nsustainable\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn appends_without_overwriting_existing_content() {
        let dir = temp_dir("no-overwrite");
        let path = dir.join("words.md");
        fs::write(&path, "# 我的词库\n已有的内容").unwrap();
        append_word(&path, "mitigate", false).unwrap();
        let content = read(&path);
        assert!(content.starts_with("# 我的词库\n已有的内容\nmitigate\n"));
        assert!(content.contains("已有的内容"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn adds_missing_trailing_newline() {
        let dir = temp_dir("missing-newline");
        let path = dir.join("words.md");
        fs::write(&path, "oldword").unwrap(); // 末尾没有换行
        append_word(&path, "newword", false).unwrap();
        assert_eq!(read(&path), "oldword\nnewword\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_extra_blank_line_when_file_ends_with_newline() {
        let dir = temp_dir("ends-newline");
        let path = dir.join("words.md");
        fs::write(&path, "oldword\n").unwrap();
        append_word(&path, "newword", false).unwrap();
        assert_eq!(read(&path), "oldword\nnewword\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedup_skips_existing_word() {
        let dir = temp_dir("dedup");
        let path = dir.join("words.md");
        assert!(append_word(&path, "mitigate", true).unwrap());
        assert!(!append_word(&path, "mitigate", true).unwrap());
        append_word(&path, "comprehensive", true).unwrap();
        assert_eq!(read(&path), "mitigate\ncomprehensive\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dedup_off_allows_repeats() {
        let dir = temp_dir("dedup-off");
        let path = dir.join("words.md");
        assert!(append_word(&path, "mitigate", false).unwrap());
        assert!(append_word(&path, "mitigate", false).unwrap());
        assert_eq!(read(&path), "mitigate\nmitigate\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_parent_directories() {
        let dir = temp_dir("parents");
        let path = dir.join("deep/nested/vocab.md");
        append_word(&path, "sustainable", false).unwrap();
        assert_eq!(read(&path), "sustainable\n");
        let _ = fs::remove_dir_all(&dir);
    }
}
