# AgentManager — AGENTS.md

> **本文件是 AI 工作协议。** Claude Code / Codex / 其他 agent 打开项目时自动加载。
> 修改本文件后**必须同步到 `CLAUDE.md`**（内容完全一致）。

## 1. 项目简介

macOS 桌面端 Claude Code 会话管理器。技术栈：**Tauri 2 + Rust + React 18 + TypeScript**。

核心数据流：
```
Claude Code hooks (SessionStart/Stop/Notification/SessionEnd)
  → ~/.claude-dashboard/hook.sh
  → HTTP POST 127.0.0.1:19280/api/notify
  → Rust axum server → DashMap state → Tauri event "session-updated"
  → React frontend (session cards / Claude history tab)
```

## 2. 模块地图

| 模块 | 职责 | SPEC | 入口 |
|------|------|------|------|
| `src-tauri/src/` | Rust 后端：状态、HTTP、iTerm 桥接、hook 安装 | `src-tauri/SPEC.md` | `lib.rs` |
| `src/` | React 前端：卡片列表、右键菜单、i18n、Claude 历史 | `src/SPEC.md` | `App.tsx` |
| `scripts/` | 构建辅助（patch-plist.sh、move-to-space.swift） | — | — |

## 3. 开发命令

```bash
npm install                # 安装依赖
npm run tauri:dev          # 开发模式（Vite HMR + Tauri）
npm run tauri:build        # Release 构建（.app + .dmg + patch Info.plist）
cargo test --lib           # Rust 单元测试
npx tsc --noEmit           # TypeScript 类型检查
```

构建产物：
- `src-tauri/target/release/bundle/macos/AgentManager.app`
- 安装：`cp -R ...AgentManager.app /Applications/`

## 4. 高优先级约束（踩坑记录）

> **以下每条都来自实际踩坑。违反会导致功能异常。AI agent 在修改相关代码前必须阅读。**

### 4.1 macOS Space（虚拟桌面）相关

| # | 约束 | 原因 |
|---|------|------|
| S1 | **arrange 必须用 System Events，不能用 `tell application "iTerm"`** | `tell application` 走 AppleEvents/Dock 层，触发 macOS 的"切换到应用所在桌面"行为，导致窗口跳到错误桌面 |
| S2 | **不能用 `set frontmost to true`，要用 `perform action "AXRaise"`** | `set frontmost` 也触发 Dock 的 Space 切换。`AXRaise` 只在当前 Space 提升窗口层级 |
| S3 | **arrange 的 AppleScript 必须用 `run_applescript_inline`（NSAppleScript 进程内执行）** | spawn `osascript` 是独立二进制，不继承 AgentManager 的辅助功能权限 → -25211 错误 |
| S4 | **`compute_region` 必须用 `current_monitor()` 而非 `primary_monitor()`** | 用户可能把 AgentManager 拖到副屏，`primary_monitor()` 返回的是主屏坐标 |
| S5 | **CGS 私有 API（`CGSMoveWindowsToManagedSpace` 等）在 macOS 14.4+ 对跨进程窗口无效** | Apple 从 14.4 起限制了跨进程 Space 移动，yabai 也需要关 SIP 才行 |
| S6 | **重装 app 后辅助功能权限失效** | macOS 按 binary hash 授权，rebuild 后 hash 变了。需 `tccutil reset Accessibility com.xiaocao.agentmanager` 或手动重新开关 |

### 4.2 iTerm AppleScript 相关

| # | 约束 | 原因 |
|---|------|------|
| I1 | **`$ITERM_SESSION_ID` 带 `wNtNpN:` 前缀，进 AppleScript 前必须 `normalize()`（strip 到 `:` 后面）** | iTerm 的 `unique id of session` 只返回裸 UUID |
| I2 | **`current tab` 和 `current session` 是只读属性，不能 `set`** | 用 `tell s to select` + `tell t to select` + `select w` 替代 |
| I3 | **`tell s to select` 不上溯切换 window** | 必须三层显式 select：`select w` → `tell t to select` → `tell s to select` |
| I4 | **jump 后用 `open -a iTerm` 而非 AppleScript `activate`** | `activate` 被 macOS 焦点抢夺防护拦截，`open -a` 走 LaunchServices 不受限 |

### 4.3 AppleScript 语法相关

| # | 约束 | 原因 |
|---|------|------|
| A1 | **AppleScript 变量不能叫 `row`** | 在 `tell process` (System Events) 块里 `row` 是保留字（UI 表格行元素），会报 -10003 |
| A2 | **`contents of variable` 不能在 `tell application "iTerm"` 块里用** | iTerm 会尝试解释为自己的对象引用 → -1728。列表操作放到 tell 块外面 |
| A3 | **arrange 的 AppleScript 里不做数学运算** | `format!` 生成的脚本在 System Events 上下文里跑 round/div/mod 会报错。所有网格计算在 Rust 里做好，AppleScript 只写字面量 |

### 4.4 前端 / Tauri 相关

| # | 约束 | 原因 |
|---|------|------|
| F1 | **不能用 `window.prompt()` / `window.alert()`** | Tauri 2 WebView 默认屏蔽。用就地编辑 `<input>` 替代 |
| F2 | **Claude Code 的 `Stop` hook ≠ 会话结束** | Stop = 一轮回答完毕（空闲），SessionEnd = 真正退出。status mapping 不能把 Stop 当 done |
| F3 | **同一 iTerm pane 切 worktree 会产生重复卡片** | 在 `upsert_from_notify` 里对 `sessionstart` 事件按 `iterm_session_id` 去重 |
| F4 | **`notification_count` 要在非 notification 事件时清零** | 否则 badge 永远涨不回来 |

## 5. Plan 工作流

**非 trivial 改动（跨模块 / 新功能 / 架构变更）必须先写 plan：**

1. 在 `docs/plans/YYYY-MM-DD/<topic>.md` 创建 plan
2. 包含：Goal、File Map、分步 Task、Verification Checklist
3. 讨论确认后再实施
4. 实施完在 plan 里勾选 checklist

格式模板见 `docs/plans/README.md`。

## 6. 知识放置规则

| 内容类型 | 放哪里 |
|----------|--------|
| 工作协议、全局约束 | `AGENTS.md`（同步到 `CLAUDE.md`） |
| Rust 模块边界、文件职责 | `src-tauri/SPEC.md` |
| React 组件边界、状态管理 | `src/SPEC.md` |
| 系统架构、数据流 | `docs/architecture/` |
| 安装、开发、调试指南 | `docs/guides/` |
| 实施计划 | `docs/plans/YYYY-MM-DD/` |

**修改 AGENTS.md 后必须同步到 CLAUDE.md。**

## 7. 验证策略

| 改动类型 | 验证要求 |
|----------|----------|
| 仅 CSS / 文案 | 无需测试，目视确认 |
| 前端逻辑 | `npx tsc --noEmit` |
| Rust 逻辑 | `cargo check` + `cargo test --lib` |
| iTerm/AppleScript | 手动测试（多桌面 + 多窗口场景） |
| 构建发布 | `npm run tauri:build` + 安装到 `/Applications` |
| 涉及辅助功能 | 必须 `tccutil reset` + 重新授权后测试 |

## 8. HTTP API

```
POST http://127.0.0.1:19280/api/notify
Content-Type: application/json

{ "session_id": "...", "cwd": "...", "iterm_session_id": "...",
  "event_type": "sessionstart|stop|notification|sessionend",
  "agent": "claude" }
→ 200 ok
```

## 9. 数据文件

| 路径 | 用途 |
|------|------|
| `~/Library/Application Support/agent-manager/sessions.json` | 持久化会话状态（最多 200 条） |
| `~/Library/Application Support/agent-manager/aliases.json` | 卡片重命名别名 |
| `~/.claude-dashboard/hook.sh` | Claude Code hook 脚本 |
| `~/.claude/settings.json` | Claude Code hooks 配置（由 install_claude_hook 写入） |
