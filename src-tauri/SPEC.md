# src-tauri/ — Rust 后端 SPEC

## 文件职责

| 文件 | 职责 | 关键约束 |
|------|------|----------|
| `lib.rs` | Tauri builder 装配、启动时窗口靠左（`dock_main_window_to_left`） | 用 `current_monitor()` 不用 `primary_monitor()` |
| `main.rs` | 入口 `fn main()` | — |
| `state.rs` | `AppState`：DashMap 会话状态 + alias 持久化 + sessions.json 读写 | SessionStart 去重（同 iterm_session_id 的旧 session 被淘汰） |
| `http_server.rs` | axum HTTP server（`127.0.0.1:19280`），`POST /api/notify` | 每次 notify 后 emit `session-updated` 事件 |
| `commands.rs` | 10 个 Tauri IPC 命令 | `arrange_iterm_windows` 不接受 `State` 参数（不再按 session 匹配窗口） |
| `iterm.rs` | iTerm AppleScript 桥接（jump / reopen / arrange） | 见 AGENTS.md §4.1–4.3 的全部约束 |
| `hook_install.rs` | `~/.claude-dashboard/hook.sh` + `~/.claude/settings.json` 幂等写入 | 5 个事件：SessionStart / UserPromptSubmit / Stop / Notification / SessionEnd |
| `claude_history.rs` | 扫描 `~/.claude/sessions/` + `~/.claude/projects/` 读历史会话 | 只读前 64KB 提取 summary，不读完整对话 |

## 数据流

```
hook.sh (stdin JSON)
  → curl POST :19280/api/notify
  → http_server::notify_handler
  → state::upsert_from_notify (去重 + 持久化)
  → app_handle.emit("session-updated")
  → 前端 listen → invoke("get_sessions") → 刷新卡片
```

## 测试

- `cargo test --lib` 跑 `iterm::tests` (normalize / parse) + `state::tests` (去重)
- 新增逻辑必须加对应的单元测试
- iTerm AppleScript 无法自动化测试，必须手动在多桌面场景验证

## 关键设计决策

1. **arrange 用 System Events 而非 iTerm AppleScript**：避免 Dock Space 切换
2. **arrange 用 NSAppleScript 进程内执行**：走 AgentManager 自己的辅助功能权限
3. **所有网格数学在 Rust 里算**：AppleScript 只写字面量坐标，无变量/数学运算
4. **`AXIsProcessTrustedWithOptions` 主动弹授权框**：不依赖用户手动去设置里找
