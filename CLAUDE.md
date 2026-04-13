# AgentManager

> Claude Code / AI agent 打开项目时自动加载的工作协议。修改后**同步到 `CLAUDE.md`**。

macOS Claude Code 会话管理器。**Tauri 2 + Rust + React 18 + TypeScript。**

## 项目结构

```
src-tauri/src/          Rust 后端（详见 src-tauri/SPEC.md）
  lib.rs                Tauri 装配 + 启动靠左
  state.rs              DashMap 会话状态 + 持久化
  http_server.rs        axum POST /api/notify :19280
  commands.rs            Tauri IPC 命令
  iterm.rs              iTerm AppleScript + System Events 桥接
  hook_install.rs       Claude hook 安装（5 个事件）
  claude_history.rs     扫描 ~/.claude/ 历史会话

src/                    React 前端（详见 src/SPEC.md）
  App.tsx               主入口（tab / session 列表 / action handler）
  components/           SessionCard / ContextMenu / ClaudeHistoryList / SetupBanner
  locales/              en.json / zh.json
```

## 开发命令

```bash
npm run tauri:dev       # 开发（HMR）
npm run tauri:build     # Release 构建
cargo test --lib        # Rust 测试
npx tsc --noEmit        # TS 类型检查
```

## ⚠️ 必读：踩坑约束

以下每条都来自实际踩坑。**改相关代码前必须看。**

### 窗口排列（arrange）

- **用 System Events 操作窗口，不能用 `tell application "iTerm"`** — 后者触发 macOS Dock 的桌面切换
- **用 `AXRaise` 逐窗口提升，不能用 `set frontmost to true`** — 后者也触发桌面切换
- **AppleScript 必须用 `run_applescript_inline`（NSAppleScript 进程内）** — spawn osascript 是独立进程，不继承 AgentManager 的辅助功能权限
- **用 `current_monitor()` 算区域，不用 `primary_monitor()`** — 用户可能在副屏
- **所有网格数学在 Rust 里算好，AppleScript 只写字面量** — format! 生成的脚本在 System Events 上下文里跑 round/div 会报错
- **变量名不能叫 `row`** — System Events 保留字，用 `gridRow`
- **重装 app 后辅助功能权限失效** — binary hash 变了，需 `tccutil reset Accessibility com.xiaocao.agentmanager`
- **CGS 私有 API 在 macOS 14.4+ 对其他进程窗口无效** — 不要再尝试 CGSMoveWindowsToManagedSpace

### iTerm 跳转（jump）

- **`$ITERM_SESSION_ID` 带 `wNtNpN:` 前缀** — 进 AppleScript 前必须 `normalize()` 取冒号后面的 UUID
- **`current tab` / `current session` 是只读的** — 用 `select w` + `tell t to select` + `tell s to select`
- **`tell s to select` 不会自动切换 window** — 三层都要显式 select
- **用 `open -a iTerm` 而非 `activate`** — activate 被焦点抢夺防护拦截
- **`contents of variable` 不能在 `tell application "iTerm"` 块里** — 列表操作放到 tell 块外

### 前端

- **不能用 `window.prompt()` / `window.alert()`** — Tauri 2 WebView 屏蔽，用 inline `<input>`
- **`Stop` hook ≠ 会话结束** — Stop = 空闲（绿色），SessionEnd = 真正结束（灰色）
- **同一 iTerm pane 切 worktree 会出重复卡片** — SessionStart 事件按 `iterm_session_id` 去重
- **`notification_count` 要在非 notification 事件时清零** — 否则 badge 只增不减

## Plan 工作流

新功能 / 跨模块改动 / 涉及 macOS 系统 API → 先在 `docs/plans/YYYY-MM-DD/<topic>.md` 写 plan，讨论确认后再实施。模板见 `docs/plans/README.md`。

纯样式、单文件 bug fix、文案翻译不需要 plan。
