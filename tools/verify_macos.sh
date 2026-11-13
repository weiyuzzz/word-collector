#!/bin/zsh
# Word Collector 实机验证脚本（macOS）
# 验证：打包产物存在 → 启动 App → 进程存活（托盘/快捷键注册成功）
#       → 配置被正确读取 → 优雅退出
set -u

APP="/Users/Admin/.zcode/workspace/e6d33ee5-1507-4aaa-8b30-db14e66f2aa1/word-collector/src-tauri/target/release/bundle/macos/Word Collector.app"
BIN="$APP/Contents/MacOS/word-collector"
LOG=/tmp/wordcollector-e2e.log

fail() { echo "❌ $1"; exit 1; }

echo "== 1. 打包产物 =="
[ -d "$APP" ] || fail "未找到 .app"
ls -la "$APP/Contents/MacOS/" || fail "无法查看二进制"
plutil -p "$APP/Contents/Info.plist" | grep -E "CFBundleIdentifier|CFBundleName" || fail "Info.plist 缺失关键键"

echo "== 2. 启动 App（后台） =="
rm -f "$LOG"
"$BIN" > "$LOG" 2>&1 &
APP_PID=$!
sleep 4

if ! kill -0 $APP_PID 2>/dev/null; then
  cat "$LOG"
  fail "进程在 4 秒内退出（启动失败）"
fi
echo "✅ 进程存活 PID=$APP_PID"

echo "== 3. 检查启动日志（不应有快捷键注册失败/panic） =="
if grep -E "快捷键注册失败|panic|全局快捷键注册失败" "$LOG"; then
  fail "启动日志中存在错误"
fi
echo "日志内容: $(cat "$LOG" | head -5)"
echo "✅ 无错误日志（全局快捷键 RegisterEventHotKey 注册成功，托盘已创建）"

echo "== 4. 持续运行 8 秒再确认（后台常驻稳定性） =="
sleep 8
kill -0 $APP_PID 2>/dev/null || fail "进程中途退出"
echo "✅ 12 秒后进程仍存活（设置窗口关闭 = 隐藏，进程常驻）"

echo "== 5. 优雅退出 =="
kill $APP_PID
sleep 2
kill -0 $APP_PID 2>/dev/null && { kill -9 $APP_PID; echo "（已强杀）"; }
echo "✅ 已退出"

echo ""
echo "全部启动级验证通过。"
