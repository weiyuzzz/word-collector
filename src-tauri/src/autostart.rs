//! 开机自启动：
//! - macOS: ~/Library/LaunchAgents/com.wordcollector.desktop.plist + launchctl
//! - Windows: HKCU\Software\Microsoft\Windows\CurrentVersion\Run 注册表项

#[cfg(target_os = "macos")]
fn set_enabled(enabled: bool) -> Result<(), String> {
    use std::process::Command;

    let home = dirs::home_dir().ok_or("无法获取用户主目录")?;
    let dir = home.join("Library").join("LaunchAgents");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 LaunchAgents 目录失败: {e}"))?;
    let plist = dir.join("com.wordcollector.desktop.plist");

    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("无法定位可执行文件: {e}"))?;
        let content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.wordcollector.desktop</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
            exe.display()
        );
        std::fs::write(&plist, content).map_err(|e| format!("写入 LaunchAgent 失败: {e}"))?;
        // 先卸载旧定义再加载，保证重复开启也不报错。
        let _ = Command::new("launchctl").args(["unload", "-w"]).arg(&plist).output();
        let out = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&plist)
            .output()
            .map_err(|e| format!("执行 launchctl 失败: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "launchctl load 失败: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(())
    } else {
        let _ = Command::new("launchctl").args(["unload", "-w"]).arg(&plist).output();
        if plist.exists() {
            std::fs::remove_file(&plist).map_err(|e| format!("删除 LaunchAgent 失败: {e}"))?;
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn set_enabled(enabled: bool) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let key = hkcu
        .open_subkey_with_flags(run_key, KEY_QUERY_VALUE | KEY_SET_VALUE)
        .or_else(|_| {
            hkcu.create_subkey(run_key)
                .map(|(k, _)| k)
                .map_err(|e| e.to_string())
        })
        .map_err(|e| format!("无法打开注册表 Run 键: {e}"))?;

    if enabled {
        let exe = std::env::current_exe().map_err(|e| format!("无法定位可执行文件: {e}"))?;
        key.set_value("Word Collector", &format!("\"{}\"", exe.display()))
            .map_err(|e| format!("写入注册表失败: {e}"))?;
    } else {
        key.delete_value("Word Collector")
            .map_err(|e| format!("删除注册表项失败: {e}"))?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn set_enabled(_enabled: bool) -> Result<(), String> {
    Err("当前平台不支持开机启动".into())
}

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    set_enabled(enabled)
}
