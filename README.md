# Word Collector

**选中英文单词 → 按全局快捷键 → 单词自动追加到 Obsidian 指定 Markdown 文件。**

不打开浏览器、不打开 Obsidian、不弹输入框、不需要手动复制粘贴、不需要联网。
核心业务全部由 Rust 后端 + 操作系统 API 完成（Tauri 2 架构），HTML/JS 界面仅承载设置项。

支持 **Windows** 与 **macOS** 双平台，已在两个平台完成实机验证。

---

## 工作原理

```
你选中单词 → 按下全局快捷键
    ↓
备份剪贴板（含图片）
    ↓
向当前前台应用合成一次复制键（Windows: Ctrl+C / macOS: ⌘C）
    ↓
轮询读取新剪贴板内容 → 校验（仅英文单词/短语）
    ↓
追加写入 Obsidian Vault 的目标 Markdown（每词一行，可选去重）
    ↓
恢复原剪贴板 → 系统通知
```

全程约 1 秒，剪贴板恢复原样，不打断你的工作流。

## 功能

| 能力 | 实现方式 |
| --- | --- |
| 全局快捷键（默认 `Ctrl+Alt+W` / `⌃⌥W`，可录制更换） | `tauri-plugin-global-shortcut`：macOS Carbon `RegisterEventHotKey`，Windows `RegisterHotKey`，后台/未聚焦均生效 |
| 获取选中文本 | OS 原生键盘模拟（Windows `SendInput`，macOS `CGEventPost`），注入前自动等待修饰键松开，避免快捷键组合键干扰 |
| 快捷键冲突检测 | 内置系统级/高频编辑键清单，录制新快捷键时实时提示 blocked / warn |
| Obsidian 写入 | 纯 Rust 文件系统追加（`OpenOptions::append`）：每词一行、不覆盖已有内容、自动补齐末尾换行、可选重复检测（整行精确匹配） |
| 后台常驻 | 原生系统托盘/菜单栏（打开设置 / 暂停采集 / 恢复采集 / 退出）；关闭设置窗口 = 隐藏窗口，进程与快捷键继续运行 |
| 系统通知 | `tauri-plugin-notification`（可关闭） |
| 开机启动 | Windows：`HKCU\...\CurrentVersion\Run` 注册表项；macOS：`~/Library/LaunchAgents/com.wordcollector.desktop.plist` + `launchctl` |
| 配置持久化 | 系统配置目录 JSON（见下文"配置文件位置"） |

---

## Windows 与 macOS 的差异

| | Windows | macOS |
| --- | --- | --- |
| 默认快捷键 | `Ctrl + Alt + W` | `⌃ Control + ⌥ Option + W` |
| 系统权限 | 无需任何额外权限 | **必须授予辅助功能权限**（系统设置 > 隐私与安全性 > 辅助功能），否则无法模拟 ⌘C，应用会明确报错引导授权 |
| 安装产物 | NSIS 安装包 `.exe` / MSI 安装包 / 独立单文件 exe | `.app`（建议拖入 /Applications）/ `.dmg` |
| 开机启动机制 | 注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` | LaunchAgent plist + `launchctl` |
| 配置文件位置 | `%APPDATA%\word-collector\config.json` | `~/Library/Application Support/word-collector/config.json` |
| 构建环境要求 | Rust (MSVC) + Visual Studio Build Tools (C++ 桌面) + Node ≥ 18；WebView2 Runtime（Win11 自带） | Rust + Xcode Command Line Tools + Node ≥ 18 |
| 托盘 | 系统托盘图标 | 菜单栏图标（template 样式，随系统深浅色适配） |

其余功能（去重、通知、暂停采集、录制快捷键、目标文件设置）两个平台完全一致。

---

## 安装

### 方式一：从源码构建（推荐，两个平台相同）

```bash
npm install
npm run tauri build
```

产物位置：

- **Windows**：
  - NSIS 安装包：`src-tauri/target/release/bundle/nsis/Word Collector_0.1.0_x64-setup.exe`
  - MSI 安装包：`src-tauri/target/release/bundle/msi/Word Collector_0.1.0_x64_en-US.msi`
  - 独立可执行文件：`src-tauri/target/release/word-collector.exe`（单文件即可运行）
- **macOS**：
  - App：`src-tauri/target/release/bundle/macos/Word Collector.app`
  - 安装包：`src-tauri/target/release/bundle/dmg/*.dmg`

可选：Windows 下用 Inno Setup 生成另一份安装包：`iscc windows\word-collector.iss`

### 方式二：直接运行单文件（Windows）

`word-collector.exe` 是独立的单文件程序，复制到任意位置双击即可运行，无需安装。

## 首次使用

### Windows

1. 运行程序（安装版或单文件均可），设置窗口弹出，托盘出现图标。
2. 在设置界面点击"选择…"指定 Obsidian Vault 目录，填写目标文件名
   （默认 `word-collector.md`，支持子目录如 `Vocabulary/words.md`）。
3. 无需任何系统权限授权。
4. 之后在任意软件选中英文单词，按 `Ctrl + Alt + W` 即可采集。

### macOS

1. 打开 App（建议拖入 `/Applications`）。
2. 在设置界面选择 Obsidian Vault 目录、填写目标文件（默认 `word-collector.md`）。
3. App 会提示辅助功能权限：点击"打开系统设置"，把 **Word Collector** 加入
   隐私与安全性 > 辅助功能，然后回到 App。
4. 之后在任意软件选中英文单词，按 `⌃ + ⌥ + W` 即可采集。

### 日常使用（两平台一致）

- **采集**：在目标应用中选中单词 → 直接按全局快捷键。注意触发瞬间前台必须是
  选词的那个应用（设置窗口本身不算）。
- **后台运行**：关闭设置窗口只是隐藏，进程与快捷键继续工作；托盘/菜单栏
  右键可打开设置、暂停采集、恢复采集、退出。
- **暂停采集**：托盘菜单勾选"暂停采集"后快捷键不写入；恢复后正常。
- **更换快捷键**：设置界面点击"点击录制"，按下新组合键即可；与系统/高频
  快捷键冲突的组合会被直接拒绝，与常见应用快捷键冲突的可确认后强制保存。

## 配置文件

JSON 配置随改随存，字段如下（均为设置界面可改项）：

```json
{
  "vaultDir": "C:\\...\\你的Vault",      // Obsidian Vault 根目录
  "targetFile": "word-collector.md",    // Vault 内相对路径，必须 .md
  "shortcut": "ctrl+alt+w",             // 规范化后的全局快捷键
  "dedupEnabled": true,                 // 重复单词检测（整行精确匹配）
  "autostart": true,                    // 开机自启动
  "notificationsEnabled": true          // 采集成功/失败的系统通知
}
```

---

## 项目结构

```
word-collector/
├── ui/                        # 极简设置界面（HTML/CSS/JS，仅设置 UI）
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/                 # icns / ico / png（tools/gen_icons.py 生成）
│   └── src/
│       ├── main.rs            # 入口：插件、全局快捷键、托盘、窗口隐藏策略
│       ├── commands.rs        # Tauri 命令 + 采集总入口 + 通知/事件
│       ├── collector.rs       # 采集管线：备份剪贴板→模拟复制→读取→恢复→校验
│       ├── markdown.rs        # Markdown 追加写入（含单元测试）
│       ├── config.rs          # 配置持久化 + 路径/快捷键规范化（含单元测试）
│       ├── shortcut_conflict.rs # 快捷键冲突检测（平台化清单）
│       ├── state.rs           # 运行时状态（暂停/计数/最近错误）
│       ├── autostart.rs       # 开机启动（注册表 / LaunchAgent）
│       ├── tray.rs            # 系统托盘菜单
│       └── platform/
│           ├── mod.rs         # 平台分发
│           ├── macos.rs       # CGEvent / AXIsProcessTrusted / Dock 显隐（手写 FFI）
│           └── windows.rs     # SendInput 合成 Ctrl+C（含修饰键等待）
├── windows/word-collector.iss # Inno Setup 安装包脚本（可选，NSIS 为默认）
└── tools/gen_icons.py         # 图标生成（纯 Python，无依赖）
```

## 自动化测试

```bash
cd src-tauri
cargo test
```

**22 个测试全部通过**（18 单元 + 4 集成），覆盖：

- 单元测试：Markdown 追加写入（三词各占一行、不覆盖已有内容、末尾换行补齐、
  去重开关、父目录自动创建）、目标文件路径校验（拒绝 `..`/绝对路径/非 .md）、
  快捷键规范化与平台化展示、快捷键冲突检测（含 Windows/macOS 专属用例）、
  选中文本校验（空文本/换行/非英文/超长拒绝）。
- 集成测试（`tests/integration.rs`）：在**真实 OS 剪贴板 + 真实文件系统**上运行
  完整采集管线（备份→复制→读取→恢复→校验→写入），验证：连续采集三个单词
  各占一行、重复去重、无新复制时报错且剪贴板不破坏、旧文件末尾无换行时
  正确补齐。与生产路径唯一差别是注入"复制动作的等效结果"替代
  CGEventPost/SendInput 键盘注入（macOS 键盘注入需要用户手工授予辅助功能
  权限，无法在无人值守环境完成）。

## 实机验证

### Windows（Windows 10 x64 + Rust 1.100 + Tauri 2，已完成）

- [x] `cargo test` 22 个测试全部通过。
- [x] `npm run tauri build` 正常产出 NSIS / MSI / 独立 exe。
- [x] 启动常驻、托盘、全局快捷键注册正常。
- [x] 真实应用中选中单词按 `Ctrl+Alt+W` 采集成功，写入目标 md（每词一行）。
- [x] 重复采集去重生效；采集后原剪贴板内容不变。
- [x] 已修复：快捷键触发时物理修饰键未松开导致目标应用收到 `Ctrl+Alt+C`
      而非 `Ctrl+C` 的问题（注入前等待 Ctrl/Alt 释放）。

### macOS（Apple Silicon / darwin 27，已完成）

```bash
# 启动级验证：打包产物 → 启动 → 托盘/快捷键注册 → 常驻 → 退出
zsh tools/verify_macos.sh
```

- [x] `npm run tauri build` 产出 `Word Collector.app` 与 `.dmg`，Info.plist 标识符正确。
- [x] 启动后进程稳定常驻（托盘与全局快捷键注册成功，无错误日志），可优雅退出。
- [x] 诊断模式在未授权辅助功能时：明确报错引导授权、**剪贴板零破坏**、不写文件。
- [x] 集成测试在真实 OS 剪贴板/文件系统上验证了完整采集管线与剪贴板恢复。

待用户首次使用时手工完成（macOS 安全设计，无法无人值守完成）：
- [ ] 在 系统设置 > 隐私与安全性 > 辅助功能 中勾选 Word Collector；
      授权后按 `⌃⌥W` 即可完成"选中单词 → 文件出现"的最后一环
      （该环已被集成测试在等效路径下覆盖）。

## 排障

### 诊断模式

```bash
# 用真实配置跑一次完整采集管线并打印结果，装好后的排障入口
word-collector.exe --test-capture          # Windows
"src-tauri/target/release/bundle/macos/Word Collector.app/Contents/MacOS/word-collector" --test-capture
```

### 诊断日志

设置环境变量 `WORD_COLLECTOR_DIAG=1` 后运行，会输出前台窗口标题、修饰键等待
时长、剪贴板轮询结果等日志，便于定位"没采到"类问题：

- 报"没有检测到新的复制内容"：多为触发瞬间前台不是选词的应用
  （例如焦点在设置窗口/桌面），或目标应用自定义了复制键位。
- macOS 报辅助功能权限错误：按界面引导去系统设置授权。

## 已知边界

- 键盘模拟发送的是标准复制键（⌘C / Ctrl+C）。个别应用自定义复制键位时无法采集。
- 触发瞬间前台必须是选词的应用；设置窗口里的"立即采集"按钮因焦点转移通常
  无法用于采集其他应用中的选中文本。
- macOS 未授予辅助功能权限时不会静默失败，会明确报错并引导授权。
- 采集依赖剪贴板中转（系统级限制），采集后会恢复原剪贴板；期间剪贴板监听类
  工具可能短暂看到复制内容。
