; Word Collector — Windows 安装包脚本（Inno Setup 6）
; 前置条件：已在 Windows 上完成 `npm run tauri build`，
; 产出 release 可执行文件位于 src-tauri\target\release\word-collector.exe
; 生成安装包：iscc windows\word-collector.iss

#define MyAppName "Word Collector"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "Word Collector"
#define MyAppExeName "word-collector.exe"

[Setup]
AppId={{8E5A2C74-9B6D-4F12-B3A1-62C97E4D5F80}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=WordCollector-{#MyAppVersion}-setup
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}
; 应用运行时不需要管理员权限（HKCU 注册表实现开机启动）
PrivilegesRequired=lowest

[Languages]
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"

[Files]
Source: "..\src-tauri\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式"; GroupDescription: "附加任务："
Name: "autostart"; Description: "开机自动启动（后台运行）"; GroupDescription: "附加任务："; Flags: unchecked

[Registry]
; 开机启动（当前用户，无需管理员权限）
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; ValueName: "Word Collector"; \
    ValueData: """{app}\{#MyAppExeName}"""; Tasks: autostart; \
    Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "启动 {#MyAppName}"; \
    Flags: nowait postinstall skipifsilent

[UninstallDelete]
; 用户词库等数据在 %APPDATA%\word-collector，卸载时保留
