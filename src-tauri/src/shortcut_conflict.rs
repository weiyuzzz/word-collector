//! 全局快捷键冲突检测。
//!
//! 操作系统没有公开 API 能枚举其他应用已注册的全局快捷键，
//! 因此采用业界通行做法：内置一份平台相关的“高危组合”清单
//! （系统级/编辑级/高频应用级），录制新快捷键时实时检查：
//! - `blocked`：与系统操作或高频编辑键冲突，拒绝保存；
//! - `warn`：与常见应用快捷键冲突，提示后允许强制保存；
//! - `ok`：无已知冲突。

use serde::Serialize;

use crate::config::{display_shortcut, normalize_shortcut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Mac,
    Windows,
    Any,
}

impl Platform {
    fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::Mac
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Any
        }
    }

    fn active_here(&self) -> bool {
        matches!(
            (self, Platform::current()),
            (Platform::Any, _) | (Platform::Mac, Platform::Mac) | (Platform::Windows, Platform::Windows)
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictReport {
    /// "ok" | "warn" | "blocked"
    pub level: String,
    pub message: Option<String>,
    pub display: String,
}

/// 修饰键顺序规范化：ctrl < meta < alt < shift，普通键放最后。
fn canonical_key(shortcut: &str) -> String {
    let normalized = normalize_shortcut(shortcut);
    let parts: Vec<&str> = normalized.split('+').collect();
    let total = parts.len();
    let (modifiers, keys): (Vec<&str>, Vec<&str>) = parts
        .into_iter()
        .partition(|p| matches!(*p, "ctrl" | "meta" | "alt" | "shift"));
    let mut ordered = Vec::with_capacity(total);
    for m in ["ctrl", "meta", "alt", "shift"] {
        if modifiers.contains(&m) {
            ordered.push(m);
        }
    }
    ordered.extend(keys);
    ordered.join("+")
}

const fn entry(combo: &'static str, platform: Platform, reason: &'static str) -> (&'static str, Platform, &'static str) {
    (combo, platform, reason)
}

/// blocked：与系统操作或文本编辑高频键冲突，保存会被拒绝。
const BLOCKED: &[(&str, Platform, &str)] = &[
    // —— 文本编辑高频键（双平台通用，Ctrl/Cmd 等价处理见下方展开） ——
    entry("ctrl+c", Platform::Any, "复制"),
    entry("ctrl+v", Platform::Any, "粘贴"),
    entry("ctrl+x", Platform::Any, "剪切"),
    entry("ctrl+z", Platform::Any, "撤销"),
    entry("ctrl+w", Platform::Any, "关闭标签页/窗口"),
    entry("ctrl+q", Platform::Any, "退出应用"),
    entry("ctrl+tab", Platform::Any, "切换标签页"),
    // —— 系统级 ——
    entry("ctrl+alt+delete", Platform::Windows, "Windows 安全界面"),
    entry("ctrl+shift+escape", Platform::Windows, "任务管理器"),
    entry("alt+f4", Platform::Windows, "关闭窗口"),
    entry("alt+tab", Platform::Windows, "切换窗口"),
    entry("meta+l", Platform::Windows, "锁定/地址栏"),
    entry("meta+space", Platform::Mac, "聚焦搜索 Spotlight"),
    entry("ctrl+space", Platform::Mac, "切换输入法"),
    entry("meta+tab", Platform::Mac, "切换应用"),
    entry("meta+`", Platform::Mac, "切换同应用窗口"),
    entry("meta+h", Platform::Mac, "隐藏窗口"),
    entry("meta+m", Platform::Mac, "最小化窗口"),
];

/// warn：与常见应用快捷键冲突，提示后允许强制保存。
const WARN: &[(&str, Platform, &str)] = &[
    entry("ctrl+a", Platform::Any, "全选"),
    entry("ctrl+s", Platform::Any, "保存"),
    entry("ctrl+f", Platform::Any, "查找"),
    entry("ctrl+n", Platform::Any, "新建"),
    entry("ctrl+t", Platform::Any, "新建标签页"),
    entry("ctrl+p", Platform::Any, "打印"),
    entry("ctrl+shift+t", Platform::Any, "恢复关闭的标签页"),
    entry("ctrl+shift+n", Platform::Any, "新建隐身窗口/文件夹"),
    entry("alt+d", Platform::Windows, "地址栏聚焦"),
    entry("meta+s", Platform::Mac, "保存"),
    entry("meta+f", Platform::Mac, "查找"),
    entry("meta+a", Platform::Mac, "全选"),
    entry("meta+w", Platform::Mac, "关闭标签页"),
    entry("meta+t", Platform::Mac, "新建标签页"),
    entry("meta+p", Platform::Mac, "打印"),
    entry("f5", Platform::Any, "刷新"),
    entry("f11", Platform::Any, "全屏切换"),
];

/// mac 的 Cmd 与 win 的 Ctrl 语义等价的键（复制/粘贴等）在 mac 上应同时
/// 匹配 "meta+c"。构建匹配键集合：给定规范化快捷键，返回它可能命中的别名。
fn aliases(shortcut: &str) -> Vec<String> {
    let canonical = canonical_key(shortcut);
    let mut result = vec![canonical.clone()];
    // mac: meta ⇄ ctrl（Cmd 与 Ctrl 在编辑语义上互为别名）
    if cfg!(target_os = "macos") {
        if let Some(stripped) = canonical.strip_prefix("ctrl+") {
            result.push(format!("meta+{stripped}"));
        }
        if let Some(stripped) = canonical.strip_prefix("meta+") {
            result.push(format!("ctrl+{stripped}"));
        }
    }
    result
}

fn scan(table: &[(&str, Platform, &str)], shortcut: &str) -> Option<String> {
    let keys = aliases(shortcut);
    for (combo, platform, reason) in table {
        if platform.active_here() && keys.iter().any(|k| k == combo) {
            return Some(format!("{}（{}）", display_shortcut(combo), reason));
        }
    }
    None
}

/// 检测给定快捷键（任意书写形式）的冲突情况。
pub fn check(shortcut: &str) -> ConflictReport {
    let display = display_shortcut(&normalize_shortcut(shortcut));

    let conflicting = scan(BLOCKED, shortcut);
    if let Some(target) = conflicting {
        return ConflictReport {
            level: "blocked".into(),
            message: Some(format!("与系统/高频快捷键冲突：{target}，请换一个组合")),
            display,
        };
    }

    let conflicting = scan(WARN, shortcut);
    if let Some(target) = conflicting {
        return ConflictReport {
            level: "warn".into(),
            message: Some(format!("可能与常用快捷键冲突：{target}，建议换一个，也可强制保存")),
            display,
        };
    }

    ConflictReport {
        level: "ok".into(),
        message: None,
        display,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_has_no_conflict() {
        assert_eq!(check("ctrl+alt+w").level, "ok");
    }

    #[test]
    fn editing_keys_are_blocked_on_every_platform() {
        // Ctrl/Cmd 语义别名：mac 上 meta+c 与 ctrl+c 等价
        let expected = if cfg!(target_os = "macos") { "meta+c" } else { "ctrl+c" };
        assert_eq!(check(expected).level, "blocked");
        assert_eq!(check("ctrl+z").level, "blocked");
        assert_eq!(check("ctrl+w").level, "blocked");
    }

    #[test]
    fn modifier_order_and_case_do_not_matter() {
        assert_eq!(check("ALT + Ctrl + W").level, "ok");
        assert_eq!(check("c+CTRL").level, "blocked");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_system_keys_blocked() {
        assert_eq!(check("meta+space").level, "blocked");
        assert_eq!(check("cmd+space").level, "blocked");
        assert_eq!(check("ctrl+space").level, "blocked");
        // mac 上 ctrl+c 应映射为 meta+c（Cmd+C 复制）→ blocked
        assert_eq!(check("ctrl+c").level, "blocked");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_warn_level() {
        assert_eq!(check("meta+s").level, "warn");
        // windows 专属键在 mac 上不命中
        assert_eq!(check("alt+f4").level, "ok");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_system_keys_blocked() {
        assert_eq!(check("alt+f4").level, "blocked");
        assert_eq!(check("ctrl+alt+delete").level, "blocked");
        assert_eq!(check("ctrl+shift+escape").level, "blocked");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_warn_level() {
        assert_eq!(check("ctrl+s").level, "warn");
    }
}
