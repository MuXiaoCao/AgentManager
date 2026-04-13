# AgentManager 系统架构

## 定位

macOS 桌面端 Claude Code 会话管理器。左侧常驻面板，实时展示运行中/历史 Claude 会话，支持跳转 iTerm、排列窗口、恢复会话。

## 技术栈

- **桌面框架**: Tauri 2.10 (Rust + WebView)
- **后端**: Rust (axum HTTP + DashMap state + AppleScript bridge)
- **前端**: React 18 + TypeScript + Vite
- **i18n**: react-i18next (en/zh)
- **窗口操作**: System Events (Accessibility API) + NSAppleScript (进程内执行)

## 整体架构

```
┌──────────────────────────────────────────────────────┐
│                    AgentManager                      │
│               (Tauri 2 desktop app)                  │
├──────────────────────────────────────────────────────┤
│                                                      │
│  React 前端              ◄─ IPC ─►    Rust 后端      │
│  ─────────                            ──────────     │
│  Tab 栏 (面板/历史)                   DashMap state   │
│  SessionCard 列表                     axum :19280     │
│  ContextMenu                          NSAppleScript   │
│  ClaudeHistoryList                    hook installer  │
│  i18n (en/zh)                         session persist │
│                                                      │
└────────────────────┬─────────────────────────────────┘
                     │ HTTP POST /api/notify
                     ▼
          ~/.claude-dashboard/hook.sh
                     ▲
                     │ stdin JSON
               Claude Code hooks
    (SessionStart / UserPromptSubmit / Stop /
     Notification / SessionEnd)
```

## 数据流

### 1. 会话接入

```
Claude Code session starts
  → SessionStart hook fires
  → hook.sh reads stdin JSON, curl POST :19280/api/notify
  → Rust: upsert_from_notify → DashMap + sessions.json
  → Tauri emit "session-updated"
  → React: listen → invoke("get_sessions") → re-render cards
```

### 2. 窗口跳转 (jump)

```
User clicks card → invoke("jump_to_iterm", {sessionId})
  → Rust: normalize iterm_session_id (strip wNtNpN: prefix)
  → osascript: tell iTerm → select w / tell t to select / tell s to select
  → Rust: open -a iTerm (force foreground via LaunchServices)
```

### 3. 窗口排列 (arrange)

```
User clicks "Arrange" → invoke("arrange_iterm_windows")
  → Rust: ensure_accessibility() (AXIsProcessTrustedWithOptions)
  → Rust: compute grid (current_monitor region → cols × rows → cell bounds)
  → NSAppleScript inline:
      tell application "System Events"
        tell process "iTerm2"
          set position of window N to {x, y}    ← 不触发 Space 切换
          set size of window N to {w, h}
          perform action "AXRaise" of window N   ← 不激活 app
        end tell
      end tell
```

### 4. 会话恢复 (reopen)

```
User clicks history card → invoke("reopen_session", {sessionId, cwd})
  → osascript: tell iTerm → create window → write text "cd <cwd> && claude --resume <id>"
  → open -a iTerm
```

## 持久化

| 文件 | 内容 | 上限 |
|------|------|------|
| `~/Library/Application Support/agent-manager/sessions.json` | 会话状态 | 200 条 |
| `~/Library/Application Support/agent-manager/aliases.json` | 卡片别名 | 无 |

## 关键设计决策

1. **System Events > iTerm AppleScript**（arrange）：避免 Dock Space 切换
2. **NSAppleScript > osascript**（arrange）：走 AgentManager 自己的 Accessibility 权限
3. **AXRaise > set frontmost**（置顶）：不触发 Space 切换
4. **current_monitor > primary_monitor**（region 计算）：支持副屏
5. **SessionStart 去重**（同 pane 切 worktree）：一个 iTerm pane 只保留最新 session
6. **5 个 hook 事件**：比原 AgentPulse 多 SessionStart + UserPromptSubmit + Notification
