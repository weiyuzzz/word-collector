// Word Collector — 主入口。
// 核心业务全部在本进程 Rust 侧完成：全局快捷键注册、键盘模拟复制、
// 剪贴板备份/恢复、Markdown 追加写入、系统托盘、通知、配置持久化。
// 前端 UI 仅承载设置项展示与修改。
// 业务模块集中在 lib.rs（供二进制与集成测试共用）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use word_collector::{autostart, collector, commands, config, state, tray};

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Builder as GlobalShortcutBuilder, ShortcutState};

fn main() {
    // 诊断模式：word-collector --test-capture
    // 不启动 UI/托盘，直接用真实配置执行一次完整采集管线并打印结果，
    // 便于排查（Vault 未配置 / 缺辅助功能权限 / 文件写入失败等）。
    if std::env::args().any(|arg| arg == "--test-capture") {
        let cfg = config::Config::load();
        match collector::capture(&cfg) {
            Ok(outcome) => {
                if outcome.appended {
                    println!("OK 已采集: {}", outcome.word);
                } else {
                    println!("OK 单词已存在: {}", outcome.word);
                }
                if let Some(path) = cfg.target_abs_path() {
                    println!("   目标文件: {}", path.display());
                }
            }
            Err(err) => {
                eprintln!("FAIL {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    let config = config::Config::load();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        // 真正的操作系统全局快捷键：macOS 走 Carbon RegisterEventHotKey，
        // Windows 走 RegisterHotKey，均在后台（无窗口/未聚焦）时生效。
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Err(err) = commands::perform_capture(app) {
                            eprintln!("[word-collector] 采集失败: {err}");
                        }
                    }
                })
                .build(),
        )
        .manage(state::AppState::new(config))
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_settings,
            commands::choose_vault,
            commands::set_shortcut,
            commands::check_shortcut_conflict,
            commands::capture_now,
            commands::pause_capture,
            commands::resume_capture,
            commands::get_runtime_status,
            commands::open_accessibility_settings,
        ])
        .setup(|app| {
            tray::create_tray(app.handle().clone())?;

            let handle = app.handle().clone();
            let state = app.state::<state::AppState>();
            let (shortcut, autostart) = {
                let guard = state.config.lock().unwrap();
                (guard.shortcut.clone(), guard.autostart)
            };
            if let Err(err) = commands::register_shortcut(&handle, &shortcut) {
                eprintln!("[word-collector] 全局快捷键注册失败: {err}");
                commands::emit_error(&handle, &format!("全局快捷键注册失败: {err}"));
            }
            // 配置里如果标记了开机启动，确保系统侧登记与之同步。
            if autostart {
                if let Err(err) = autostart::set_autostart(true) {
                    eprintln!("[word-collector] 开机启动登记失败: {err}");
                }
            }
            Ok(())
        })
        // 关闭设置窗口 = 隐藏窗口，进程与全局快捷键继续运行。
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                word_collector::platform::macos::set_dock_icon_visible(false);
            }
        })
        .run(tauri::generate_context!())
        .expect("Word Collector 启动失败");
}
