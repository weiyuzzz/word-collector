# Word Collector

选中英文单词 → 按全局快捷键 → 单词自动追加到 Obsidian 指定 Markdown 文件。

不打开浏览器、不打开 Obsidian、不弹输入框、不需要手动复制粘贴、不需要联网。
核心业务全部由 Rust 后端 + 操作系统 API 完成（Tauri 2 架构），HTML/JS 界面仅承载设置项。

## 功能

| 能力 | 实现方式 |
| --- | --- |
| 全局快捷键（默认 `Ctrl+Alt+W` / `⌃⌥W`） | `tauri-plugin-global-shortcut`：macOS Carbon `RegisterEventHotKey`，Windows `RegisterHotKey`，后台/未聚焦均生效，可在设置界面录制新快捷键 |
| 获取选中文本 | 备份剪贴板（含图片内容）→ OS 原生键盘模拟（macOS `CGEventPost` 合成 ⌘C；Windows `SendInput` 合成 Ctrl+C）→ 读取新文本 → 恢复原剪贴板 |
| macOS 辅助功能权限 | `AXIsProcessTrusted()` 检测；未授权时界面显示横幅并可一键跳转 系统设置 > 隐私与安全性 > 辅助功能 |
| Obsidian 写入 | 纯 Rust 文件系统追加（`OpenOptions::append`）：每词一行、不覆盖已有内容、自动补齐末尾换行、可选重复检测（整行精确匹配） |
| 后台运行 | 原生系统托盘/菜单栏图标（打开设置 / 暂停采集 / 恢复采集 / 退出）；关闭设置窗口 = 隐藏窗口，进程与快捷键继续运行 |
| 系统通知 | `tauri-plugin-notification`（可关闭） |
| 开机启动 | macOS：`~/Library/LaunchAgents/com.wordcollector.desktop.plist` + `launchctl`；Windows：`HKCU\...\CurrentVersion\Run` 注册表项 |
| 配置持久化 | 系统配置目录 JSON（macOS `~/Library/Application Support/word-collector/config.json`；Windows `%APPDATA%\word-collector\config.json`） |

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
│       ├── state.rs           # 运行时状态（暂停/计数/最近错误）
│       ├── autostart.rs       # 开机启动（LaunchAgent / 注册表）
│       ├── tray.rs            # 系统托盘菜单
│       └── platform/
│           ├── mod.rs         # 平台分发
│           ├── macos.rs       # CGEvent / AXIsProcessTrusted / Dock 显隐（手写 FFI）
│           └── windows.rs     # SendInput 合成 Ctrl+C
├── windows/word-collector.iss # Inno Setup 安装包脚本（可选，NSIS 为默认）
└── tools/gen_icons.py         # 图标生成（纯 Python，无依赖）
```

## macOS 构建与安装

要求：Rust（rustup stable）、Node.js ≥ 18、Xcode Command Line Tools。

```bash
npm install
npm run tauri build
```

产物：
- App：`src-tauri/target/release/bundle/macos/Word Collector.app`
- 安装包：`src-tauri/target/release/bundle/dmg/*.dmg`

首次使用：
1. 打开 App（建议拖入 `/Applications`）。
2. 在设置界面选择 Obsidian Vault 目录、填写目标文件（默认 `word-collector.md`）。
3. App 会提示辅助功能权限：点击“打开系统设置”，把 **Word Collector** 加入
   隐私与安全性 > 辅助功能，然后回到 App。
4. 之后在任意软件选中英文单词，按 `⌃ + ⌥ + W` 即可采集。

> 说明：本机构建产物为 ad-hoc 签名；正式分发需要开发者证书时，
> 在 `tauri.conf.json` 的 `bundle.macOS` 中配置签名/公证参数后重新构建。

## Windows 构建与安装

要求：Rust（rustup stable, MSVC target）、Node.js ≥ 18、Visual Studio Build Tools（C++ 桌面）、WebView2 Runtime（Win11 自带）。

```bash
npm install
npm run tauri build
```

产物：
- NSIS 安装包：`src-tauri/target/release/bundle/nsis/Word Collector_0.1.0_x64-setup.exe`
- 独立可执行文件：`src-tauri/target/release/word-collector.exe`（单文件即可运行）

可选：使用 Inno Setup 生成另一个安装包：

```bat
iscc windows\word-collector.iss
```

Windows 上无需额外系统权限；快捷键为 `Ctrl + Alt + W`。

## 自动化测试

```bash
cd src-tauri
cargo test
```

**17 个测试全部通过**（13 单元 + 4 集成），覆盖：

- 单元测试：Markdown 追加写入（三词各占一行、不覆盖已有内容、末尾换行补齐、
  去重开关、父目录自动创建）、目标文件路径校验（拒绝 `..`/绝对路径/非 .md）、
  快捷键规范化与平台化展示、选中文本校验（空文本/换行/非英文/超长拒绝）。
- 集成测试（`tests/integration.rs`）：在**真实 OS 剪贴板 + 真实文件系统**上运行
  完整采集管线（备份→复制→读取→恢复→校验→写入），验证：连续采集三个单词
  各占一行、重复去重、无新复制时报错且剪贴板不破坏、旧文件末尾无换行时
  正确补齐。与生产路径唯一差别是注入"复制动作的等效结果"替代
  CGEventPost/SendInput 键盘注入（键盘注入需要用户手工授予辅助功能权限，
  无法在无人值守环境完成）。

## 实机验证

### macOS（本交付已完成，Apple Silicon / darwin 27）

```bash
# 启动级验证：打包产物 → 启动 → 托盘/快捷键注册 → 常驻 → 退出
zsh tools/verify_macos.sh

# 诊断模式：用真实配置跑一次完整采集管线（装好后的排障入口）
"src-tauri/target/release/bundle/macos/Word Collector.app/Contents/MacOS/word-collector" --test-capture
```

已验证：
- [x] `npm run tauri build` 产出 `Word Collector.app`（5.27 MiB）与 `.dmg`（2.22 MiB），Info.plist 标识符正确。
- [x] 启动后进程稳定常驻（托盘与全局快捷键注册成功，无错误日志），可优雅退出。
- [x] 诊断模式在未授权辅助功能时：明确报错引导授权、**剪贴板零破坏**、不写文件。
- [x] 集成测试在真实 OS 剪贴板/文件系统上验证了完整采集管线与剪贴板恢复。

待用户首次使用时手工完成（macOS 安全设计，无法无人值守完成）：
- [ ] 在 系统设置 > 隐私与安全性 > 辅助功能 中勾选 Word Collector；
      授权后按 `⌃⌥W` 即可完成"选中单词 → 文件出现"的最后一环
      （该环已被集成测试在等效路径下覆盖）。

### Windows（代码结构完整、可编译，未在 Windows 实机验证）

在 Windows 机器上按上文"构建与安装"构建后，逐项执行以下验收清单：

- [ ] 任意应用选中英文单词，按 `Ctrl+Alt+W`，Obsidian 目标 md 文件出现该单词（独占一行）。
- [ ] 连续采集多个单词，按顺序多行追加。
- [ ] 已有内容（末尾无换行）不被破坏，新词另起一行。
- [ ] 采集后原剪贴板内容不变。
- [ ] 关闭设置窗口后快捷键仍有效（托盘图标在、进程在）。
- [ ] 托盘"暂停采集"后快捷键不写入；恢复后正常。
- [ ] 录制新快捷键立即生效；开机启动开关生效（HKCU Run）。

## 已知边界

- 键盘模拟发送的是标准复制键（⌘C / Ctrl+C）。个别应用自定义复制键位时无法采集。
- macOS 未授予辅助功能权限时不会静默失败，会明确报错并引导授权。
- 采集依赖剪贴板中转（系统级限制），采集后会恢复原剪贴板；期间剪贴板监听类工具可能短暂看到复制内容。
