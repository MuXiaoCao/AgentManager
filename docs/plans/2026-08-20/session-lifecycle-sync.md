# 历史会话唤起与退出状态同步

**Goal:** 历史会话恢复后立即出现在“正在使用”，Claude/Codex 会话退出后立即从“正在使用”移入历史。

**Tech Stack / 影响范围:** Rust 状态管理、iTerm AppleScript 桥接、Claude/Codex hooks、Tauri IPC。

---

## 改造概览

```text
历史恢复 -> 创建 iTerm window 并取得 session ID -> 立即 upsert active card
                                                     |
真实 SessionStart/UserPromptSubmit ------------------+

CLI exit -> SessionEnd hook -> /api/notify -> last_event=sessionend -> 历史区
```

---

## Task 1: 恢复历史会话时立即激活卡片

**Files:**

- Modify: `src-tauri/src/iterm.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/state.rs`

- [x] iTerm 创建窗口后返回新 pane 的 session ID。
- [x] `reopen_session` 成功后立即写入活跃状态，保留 agent/cwd/session_id。
- [x] 真实 hook 到达后继续覆盖临时状态。

---

## Task 2: 用 SessionEnd 同步退出状态

**Files:**

- Modify: `src-tauri/src/hook_install.rs`
- Modify: `src/App.tsx`
- Modify: `src/components/SetupBanner.tsx`

- [x] 将 Codex `SessionEnd` 纳入安装与健康检查。
- [x] AgentManager 的 SessionEnd hook timeout 保持在 Codex 允许范围内。
- [x] 更新前端必需事件列表，提示已有用户补装 hook。

---

## Task 3: 迁移 Codex feature flag

**Files:**

- Modify: `src-tauri/src/hook_install.rs`

- [x] 安装 hooks 时将 `[features].codex_hooks` 安全迁移为 `hooks`。
- [x] 保留用户其他 config.toml 内容。
- [x] 补充迁移单元测试。

---

## Verification Checklist

- [x] `cargo test --lib` 通过
- [x] `npx tsc --noEmit` 通过
- [x] 历史会话恢复后无需首次输入即可写入活跃状态
- [x] Codex SessionEnd 已安装，退出事件可将卡片移入历史
- [x] `codex_hooks` 警告消失
- [x] 不覆盖用户已有的其他 hooks
