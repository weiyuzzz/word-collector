//! 配置持久化：JSON 存放在系统配置目录
//! （macOS: ~/Library/Application Support/word-collector/config.json
//!  Windows: %APPDATA%/word-collector/config.json）。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_SHORTCUT: &str = "ctrl+alt+w";
pub const DEFAULT_TARGET_FILE: &str = "word-collector.md";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Obsidian Vault 根目录。
    pub vault_dir: Option<PathBuf>,
    /// Vault 内的相对路径，如 "word-collector.md" 或 "Vocabulary/words.md"。
    pub target_file: String,
    /// 规范化后的全局快捷键，如 "ctrl+alt+w"。
    pub shortcut: String,
    /// 重复单词检测开关。
    pub dedup_enabled: bool,
    /// 开机自启动。
    pub autostart: bool,
    /// 系统通知开关。
    pub notifications_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vault_dir: None,
            target_file: DEFAULT_TARGET_FILE.to_string(),
            shortcut: DEFAULT_SHORTCUT.to_string(),
            dedup_enabled: true,
            autostart: false,
            notifications_enabled: true,
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("word-collector")
            .join("config.json")
    }

    pub fn load() -> Config {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str(&raw) {
                Ok(cfg) => cfg,
                Err(err) => {
                    eprintln!(
                        "[word-collector] 配置文件解析失败（{}），使用默认配置: {err}",
                        path.display()
                    );
                    Config::default()
                }
            },
            Err(_) => Config::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
        }
        let raw = serde_json::to_string_pretty(self).map_err(|e| format!("配置序列化失败: {e}"))?;
        fs::write(&path, raw).map_err(|e| format!("写入配置文件失败: {e}"))
    }

    /// 目标 Markdown 文件的绝对路径（Vault 目录 + 相对路径）。
    pub fn target_abs_path(&self) -> Option<PathBuf> {
        self.vault_dir
            .as_ref()
            .map(|vault| vault.join(&self.target_file))
    }

    pub fn shortcut_display(&self) -> String {
        display_shortcut(&self.shortcut)
    }
}

/// 校验并规范化用户输入的目标文件路径（Vault 内相对路径）。
pub fn sanitize_target_file(input: &str) -> Result<String, String> {
    let normalized = input.trim().replace('\\', "/");
    if normalized.is_empty() {
        return Err("目标文件不能为空".into());
    }
    if normalized.starts_with('/') {
        return Err("目标文件必须是 Vault 内的相对路径".into());
    }
    for part in normalized.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("目标文件路径包含非法片段: “{part}”"));
        }
    }
    if !normalized.to_lowercase().ends_with(".md") {
        return Err("目标文件必须是 .md 文件".into());
    }
    Ok(normalized)
}

/// 把 "Ctrl + Alt + W" / "cmd+space" 这类输入规范化为 "ctrl+alt+w" 形式，
/// 同时归一别名：cmd/command/win/super → meta，option/opt → alt，control → ctrl。
pub fn normalize_shortcut(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|part| match part {
            "cmd" | "command" | "super" | "win" => "meta",
            "option" | "opt" => "alt",
            "control" => "ctrl",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// 按当前平台生成人类可读的快捷键展示文案。
pub fn display_shortcut(shortcut: &str) -> String {
    shortcut
        .split('+')
        .map(|part| match part {
            "ctrl" => {
                if cfg!(target_os = "macos") {
                    "⌃ Control".to_string()
                } else {
                    "Ctrl".to_string()
                }
            }
            "alt" => {
                if cfg!(target_os = "macos") {
                    "⌥ Option".to_string()
                } else {
                    "Alt".to_string()
                }
            }
            "shift" => {
                if cfg!(target_os = "macos") {
                    "⇧ Shift".to_string()
                } else {
                    "Shift".to_string()
                }
            }
            "meta" => {
                if cfg!(target_os = "macos") {
                    "⌘ Command".to_string()
                } else {
                    "Win".to_string()
                }
            }
            other => other.to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_accepts_nested_md_path() {
        assert_eq!(
            sanitize_target_file("Vocabulary\\words.md").unwrap(),
            "Vocabulary/words.md"
        );
    }

    #[test]
    fn sanitize_rejects_traversal_and_absolute() {
        assert!(sanitize_target_file("../evil.md").is_err());
        assert!(sanitize_target_file("a/../../b.md").is_err());
        assert!(sanitize_target_file("/etc/passwd.md").is_err());
        assert!(sanitize_target_file("notes.txt").is_err());
        assert!(sanitize_target_file("").is_err());
    }

    #[test]
    fn normalize_handles_spaces_and_case() {
        assert_eq!(normalize_shortcut(" Ctrl + Alt + W "), "ctrl+alt+w");
        assert_eq!(normalize_shortcut("ctrl+alt+w"), "ctrl+alt+w");
        assert_eq!(normalize_shortcut("cmd+space"), "meta+space");
        assert_eq!(normalize_shortcut("control+option+w"), "ctrl+alt+w");
        assert_eq!(normalize_shortcut("Command+Shift+P"), "meta+shift+p");
    }

    #[test]
    fn display_shows_platform_friendly_text() {
        let shown = display_shortcut("ctrl+alt+w");
        assert!(shown.contains('W'));
        if cfg!(target_os = "macos") {
            assert!(shown.contains("⌃"));
        } else {
            assert!(shown.contains("Ctrl"));
        }
    }
}
