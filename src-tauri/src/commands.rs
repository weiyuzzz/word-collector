//! Tauri 命令层 + 采集/通知/事件广播的共享入口。
//! 全局快捷键与托盘都调用 perform_capture；设置界面通过命令读写配置。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;

use crate::autostart;
use crate::collector;
use crate::config::{normalize_shortcut, sanitize_target_file, Config};
use crate::platform;
use crate::shortcut_conflict;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub paused: bool,
    pub collected: usize,
    pub last_error: Option<String>,
    pub target_path: Option<String>,
    pub shortcut_display: String,
    pub accessibility_granted: bool,
}

fn emit_event(
    app: &AppHandle,
    kind: &str,
    word: Option<&str>,
    appended: Option<bool>,
    message: Option<&str>,
    total: usize,
) {
    let _ = app.emit(
        "collector-event",
        serde_json::json!({
            "kind": kind,
            "word": word,
            "appended": appended,
            "message": message,
            "total": total,
        }),
    );
}

pub fn emit_error(app: &AppHandle, message: &str) {
    emit_event(app, "error", None, None, Some(message), 0);
}

fn emit_status_changed(app: &AppHandle) {
    let _ = app.emit("status-changed", serde_json::json!({}));
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// 采集总入口：全局快捷键、托盘、设置界面“立即采集”全部走这里。
pub fn perform_capture(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    if state.is_paused() {
        let msg = "采集已暂停";
        emit_event(app, "error", None, None, Some(msg), state.collected());
        return Err(msg.into());
    }

    let cfg = state.config.lock().unwrap().clone();

    // 串行化采集：快速连按快捷键时避免两个管线并发争抢剪贴板。
    let _capture_guard = state.acquire_capture_lock();

    match collector::capture(&cfg) {
        Ok(outcome) => {
            let total = state.record_collected();
            state.set_last_error(None);
            if cfg.notifications_enabled {
                let title = if outcome.appended { "已采集单词" } else { "单词已存在" };
                let body = if outcome.appended {
                    format!("“{}” 已追加到词库", outcome.word)
                } else {
                    format!("“{}” 已在词库中，未重复追加", outcome.word)
                };
                notify(app, title, &body);
            }
            emit_event(
                app,
                "captured",
                Some(&outcome.word),
                Some(outcome.appended),
                None,
                total,
            );
            Ok(outcome.word)
        }
        Err(err) => {
            state.set_last_error(Some(err.clone()));
            if cfg.notifications_enabled {
                notify(app, "采集失败", &err);
            }
            emit_event(app, "error", None, None, Some(&err), state.collected());
            Err(err)
        }
    }
}

pub fn toggle_pause(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let paused = !state.is_paused();
    state.set_paused(paused);
    emit_status_changed(app);
    paused
}

pub fn show_settings(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    crate::platform::macos::set_dock_icon_visible(true);
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 注册（或替换）操作系统全局快捷键。旧快捷键会被注销。
pub fn register_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

    let parsed =
        Shortcut::from_str(shortcut).map_err(|err| format!("无效的快捷键 \"{shortcut}\": {err}"))?;
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|err| format!("注销旧快捷键失败: {err}"))?;
    manager
        .register(parsed)
        .map_err(|err| format!("注册快捷键失败: {err}"))?;
    Ok(())
}

// ---------- Tauri 命令 ----------

#[tauri::command]
pub fn get_config(state: State<AppState>) -> Config {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<AppState>,
    dedup_enabled: Option<bool>,
    notifications_enabled: Option<bool>,
    autostart: Option<bool>,
    target_file: Option<String>,
) -> Result<Config, String> {
    let mut guard = state.config.lock().unwrap();
    if let Some(v) = dedup_enabled {
        guard.dedup_enabled = v;
    }
    if let Some(v) = notifications_enabled {
        guard.notifications_enabled = v;
    }
    if let Some(v) = autostart {
        if v != guard.autostart {
            // 系统侧登记失败时直接报错，不落配置。
            autostart::set_autostart(v)?;
            guard.autostart = v;
        }
    }
    if let Some(v) = target_file {
        guard.target_file = sanitize_target_file(&v)?;
    }
    guard.save()?;
    drop(guard);
    emit_status_changed(&app);
    Ok(state.config.lock().unwrap().clone())
}

#[tauri::command]
pub fn choose_vault(app: AppHandle) -> Result<(), String> {
    let callback_app = app.clone();
    app.dialog()
        .file()
        .set_title("选择 Obsidian Vault 目录")
        .pick_folder(move |path| {
            let Some(path) = path else { return };
            let dir = match path.into_path() {
                Ok(dir) => dir,
                Err(err) => {
                    eprintln!("[word-collector] 选择的目录无效: {err}");
                    return;
                }
            };
            let state = callback_app.state::<AppState>();
            {
                let mut guard = state.config.lock().unwrap();
                guard.vault_dir = Some(dir);
                if let Err(err) = guard.save() {
                    eprintln!("[word-collector] 保存配置失败: {err}");
                }
            }
            emit_status_changed(&callback_app);
        });
    Ok(())
}

#[tauri::command]
pub fn check_shortcut_conflict(shortcut: String) -> shortcut_conflict::ConflictReport {
    shortcut_conflict::check(&shortcut)
}

#[tauri::command]
pub fn set_shortcut(app: AppHandle, state: State<AppState>, shortcut: String) -> Result<Config, String> {
    let normalized = normalize_shortcut(&shortcut);
    if normalized.is_empty() {
        return Err("快捷键不能为空".into());
    }
    // 冲突检测：与系统操作/高频编辑键冲突的组合直接拒绝保存。
    let report = shortcut_conflict::check(&normalized);
    if report.level == "blocked" {
        return Err(
            report
                .message
                .unwrap_or_else(|| "该快捷键与系统/高频快捷键冲突".into()),
        );
    }
    // 先注册验证，成功才落配置。
    register_shortcut(&app, &normalized)?;
    {
        let mut guard = state.config.lock().unwrap();
        guard.shortcut = normalized;
        guard.save()?;
    }
    emit_status_changed(&app);
    Ok(state.config.lock().unwrap().clone())
}

#[tauri::command]
pub fn capture_now(app: AppHandle) -> Result<String, String> {
    perform_capture(&app)
}

#[tauri::command]
pub fn pause_capture(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.set_paused(true);
    emit_status_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn resume_capture(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.set_paused(false);
    state.set_last_error(None);
    emit_status_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn get_runtime_status(state: State<AppState>) -> RuntimeStatus {
    let cfg = state.config.lock().unwrap().clone();
    RuntimeStatus {
        paused: state.is_paused(),
        collected: state.collected(),
        last_error: state.last_error(),
        target_path: cfg.target_abs_path().map(|p| p.display().to_string()),
        shortcut_display: cfg.shortcut_display(),
        accessibility_granted: platform::accessibility_granted(),
    }
}

#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    platform::open_accessibility_settings()
}
