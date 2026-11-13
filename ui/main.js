// 设置界面逻辑：仅调用 Rust 后端命令（window.__TAURI__，withGlobalTauri），
// 所有真实功能（快捷键注册、剪贴板、文件写入、托盘）都在 Rust + OS 层。
// 注意：这里的键盘事件只用于“录制新快捷键”这一配置行为，
// 全局快捷键监听本身是操作系统级的，与网页事件无关。

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
const isMac = navigator.userAgent.includes("Mac");
let recording = false;
let pendingCombo = null; // 冲突警告（warn）下待用户确认的快捷键

function addLog(text, isError = false) {
  const list = $("capture-log");
  const item = document.createElement("li");
  if (isError) item.className = "error";
  item.textContent = text;
  list.prepend(item);
  while (list.children.length > 30) list.removeChild(list.lastChild);
}

async function refresh() {
  try {
    const [cfg, status] = await Promise.all([
      invoke("get_config"),
      invoke("get_runtime_status"),
    ]);
    $("vault-path").textContent = cfg.vaultDir || "未设置（点击“选择…”指定 Obsidian Vault）";
    $("target-file").value = cfg.targetFile;
    $("opt-dedup").checked = cfg.dedupEnabled;
    $("opt-notify").checked = cfg.notificationsEnabled;
    $("opt-autostart").checked = cfg.autostart;
    $("shortcut-text").textContent = `按 ${status.shortcutDisplay} 采集选中单词`;
    $("shortcut-hint").textContent = `当前：${status.shortcutDisplay}`;
    $("collected-count").textContent = `${status.collected} 个`;
    $("status-dot").classList.toggle("paused", status.paused);
    $("status-text").textContent = status.paused ? "已暂停" : "运行中";
    $("btn-pause").textContent = status.paused ? "恢复采集" : "暂停采集";
    $("btn-pause").dataset.paused = status.paused ? "1" : "0";
    if (status.lastError) addLog(`出错：${status.lastError}`, true);
    // 辅助功能权限横幅仅 macOS 显示
    $("ax-banner").classList.toggle("hidden", !isMac || status.accessibilityGranted);
    $("shortcut-rec").textContent = "点击录制";
    $("shortcut-rec").classList.remove("recording");
    $("shortcut-force").classList.add("hidden");
    $("shortcut-hint").classList.remove("err", "warn");
    if (!recording) $("shortcut-hint").textContent = `当前：${status.shortcutDisplay}`;
    pendingCombo = null;
    recording = false;
  } catch (err) {
    addLog(`读取状态失败：${err}`, true);
  }
}

// ---------- 事件（来自 Rust 后端） ----------
listen("collector-event", (event) => {
  const p = event.payload;
  if (p.kind === "captured") {
    addLog(p.appended ? `已采集：${p.word}` : `已存在：${p.word}`);
  } else {
    addLog(`出错：${p.message}`, true);
  }
  refresh();
});
listen("status-changed", refresh);

// ---------- 操作绑定 ----------
$("btn-pause").addEventListener("click", async () => {
  try {
    await invoke($("btn-pause").dataset.paused === "1" ? "resume_capture" : "pause_capture");
  } catch (err) {
    addLog(`操作失败：${err}`, true);
  }
  refresh();
});

$("btn-choose-vault").addEventListener("click", async () => {
  try {
    await invoke("choose_vault");
  } catch (err) {
    addLog(`选择目录失败：${err}`, true);
  }
});

$("target-file").addEventListener("change", async (e) => {
  try {
    await invoke("update_settings", { targetFile: e.target.value });
  } catch (err) {
    addLog(`保存失败：${err}`, true);
  }
  refresh();
});

$("opt-dedup").addEventListener("change", (e) =>
  invoke("update_settings", { dedupEnabled: e.target.checked }).then(refresh)
);
$("opt-notify").addEventListener("change", (e) =>
  invoke("update_settings", { notificationsEnabled: e.target.checked }).then(refresh)
);
$("opt-autostart").addEventListener("change", async (e) => {
  try {
    await invoke("update_settings", { autostart: e.target.checked });
  } catch (err) {
    addLog(`设置开机启动失败：${err}`, true);
    refresh();
  }
});

$("btn-capture-now").addEventListener("click", async () => {
  try {
    const word = await invoke("capture_now");
    addLog(`已采集：${word}`);
  } catch (err) {
    addLog(`采集失败：${err}`, true);
  }
  refresh();
});

$("btn-open-ax").addEventListener("click", () => invoke("open_accessibility_settings"));

// ---------- 快捷键录制（仅写入配置，注册由 Rust 完成） ----------
$("shortcut-rec").addEventListener("click", () => {
  if (recording) return;
  recording = true;
  $("shortcut-rec").textContent = "请按下新的快捷键…";
  $("shortcut-rec").classList.add("recording");
});

const PUNCT_KEYS = {
  Comma: ",", Period: ".", Minus: "-", Equal: "=", Slash: "/", Backquote: "`",
  Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]",
};

function keyFromEvent(e) {
  const code = e.code || "";
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  if (code.startsWith("Digit")) return code.slice(5);
  if (PUNCT_KEYS[code]) return PUNCT_KEYS[code];
  const k = (e.key || "").toLowerCase();
  return /^[a-z0-9]$/.test(k) ? k : null;
}

window.addEventListener("keydown", async (e) => {
  if (!recording) return;
  e.preventDefault();
  e.stopPropagation();

  if (e.key === "Escape") {
    recording = false;
    $("shortcut-rec").textContent = "点击录制";
    $("shortcut-rec").classList.remove("recording");
    return;
  }

  const parts = [];
  if (e.ctrlKey) parts.push("ctrl");
  if (e.metaKey) parts.push("meta");
  if (e.altKey) parts.push("alt");
  if (e.shiftKey) parts.push("shift");
  const key = keyFromEvent(e);
  if (!key || parts.length === 0) return; // 必须包含至少一个修饰键

  const combo = [...parts, key].join("+");

  try {
    // 冲突检测：blocked 拒绝保存；warn 提示后允许强制保存；ok 直接保存
    const report = await invoke("check_shortcut_conflict", { shortcut: combo });
    if (report.level === "blocked") {
      $("shortcut-hint").textContent = `⛔ ${report.message}`;
      $("shortcut-hint").classList.add("err");
      addLog(`快捷键 ${combo} 被拒绝：${report.message}`, true);
      recording = false;
      $("shortcut-rec").textContent = "点击录制";
      $("shortcut-rec").classList.remove("recording");
      return;
    }
    if (report.level === "warn") {
      pendingCombo = combo;
      $("shortcut-hint").textContent = `⚠️ ${report.message}`;
      $("shortcut-hint").classList.add("warn");
      $("shortcut-force").classList.remove("hidden");
      return;
    }
    await saveShortcut(combo, report.display);
  } catch (err) {
    addLog(`快捷键设置失败：${err}`, true);
    recording = false;
    $("shortcut-rec").textContent = "点击录制";
    $("shortcut-rec").classList.remove("recording");
  }
});

async function saveShortcut(combo, display) {
  await invoke("set_shortcut", { shortcut: combo });
  addLog(`全局快捷键已更新为 ${display || combo}`);
  recording = false;
  refresh();
}

$("shortcut-force").addEventListener("click", async () => {
  if (!pendingCombo) return;
  try {
    await saveShortcut(pendingCombo);
  } catch (err) {
    addLog(`快捷键设置失败：${err}`, true);
    refresh();
  }
});

window.addEventListener("focus", refresh); // macOS 授权回来后刷新权限状态
refresh();
