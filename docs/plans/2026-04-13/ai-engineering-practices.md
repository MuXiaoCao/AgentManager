# 引入 AI 工程实践体系

> **For agentic workers:** 本 plan 参照 RewindDesktop 的工程实践，适配 AgentManager 项目规模。

**Goal:** 建立 AGENTS.md + Plan 工作流 + 文档体系，让后续开发（无论人类还是 AI）有据可循。

**参考来源:** `~/IdeaProjects/RewindDesktop/` 的 AGENTS.md、docs/plans/、.governance/ 设计。

**适配原则:** AgentManager 是中小型项目（~2K LOC），不需要 RewindDesktop 的全部治理机制（34 个 skills、governance dream 等）。取其精华：**AGENTS.md harness + Plan 工作流 + 分层文档**。

---

## 改造概览

```
AgentManager/
├── AGENTS.md              ← 新增：仓库入口 & AI 工作协议
├── CLAUDE.md              ← 新增：AGENTS.md 的镜像（Claude Code 自动加载）
├── docs/
│   ├── README.md          ← 新增：文档中心索引
│   ├── architecture/
│   │   └── overview.md    ← 新增：系统架构 & 数据流
│   ├── guides/
│   │   └── setup.md       ← 新增：安装 & 开发指南（从 README.md 抽出）
│   └── plans/
│       ├── README.md      ← 新增：plan 格式规范
│       └── 2026-04-13/
│           └── ai-engineering-practices.md  ← 本文件
├── src-tauri/
│   └── SPEC.md            ← 新增：Rust 后端模块边界
└── src/
    └── SPEC.md            ← 新增：React 前端模块边界
```

---

## Task 1: 创建 AGENTS.md（核心 harness）

**Purpose:** Claude Code / Codex / 其他 AI agent 打开项目时自动加载的工作协议。

**Files:**
- Create: `AGENTS.md`
- Create: `CLAUDE.md`（内容 = `AGENTS.md` 的完整镜像）

**内容结构（参照 RewindDesktop 但精简）：**

```markdown
# AgentManager — AGENTS.md

## 1. 项目简介
- 一句话定位 + 技术栈
- 核心交互：hook.sh → HTTP /api/notify → DashMap → Tauri IPC → React

## 2. 模块地图
| 模块 | 职责 | SPEC |
|------|------|------|
| src-tauri/src/ | Rust 后端 | src-tauri/SPEC.md |
| src/ | React 前端 | src/SPEC.md |
| scripts/ | 构建辅助 | — |

## 3. 开发命令
npm run tauri:dev / tauri:build / etc.

## 4. 高优先级约束
- System Events 不能用 `set frontmost`（会跳桌面）
- iTerm session id 要 normalize（strip wNtNpN: 前缀）
- AppleScript 里不能有变量名 `row`（System Events 保留字）
- 排列相关 AppleScript 必须用 NSAppleScript 进程内执行（不能 spawn osascript）
- ...（从我们踩过的坑里提炼）

## 5. Plan 工作流
非 trivial 改动必须先在 docs/plans/YYYY-MM-DD/ 下写 plan

## 6. 知识放置规则
- AGENTS.md: 工作协议（必须同步到 CLAUDE.md）
- src-tauri/SPEC.md: Rust 模块边界
- src/SPEC.md: React 组件边界
- docs/plans/: 实施计划
- docs/architecture/: 架构设计
- docs/guides/: 操作手册

## 7. 验证策略
- cargo check + tsc --noEmit: 每次改完跑
- cargo test --lib: 有新逻辑时跑
- tauri build: 提交前跑
- 手动测试: arrange 必须在多桌面场景下验证
```

---

## Task 2: 创建模块 SPEC.md

**Files:**
- Create: `src-tauri/SPEC.md`
- Create: `src/SPEC.md`

**src-tauri/SPEC.md 内容：**
- 各 .rs 文件的职责一句话说明
- 关键约束（NSAppleScript vs osascript、CGS API 不可用、etc.）
- 数据流：hook.sh → HTTP → state → IPC → frontend

**src/SPEC.md 内容：**
- 组件树：App → SessionCard / ContextMenu / ClaudeHistoryList / SetupBanner
- i18n 结构
- 状态管理：useState + Tauri event 驱动

---

## Task 3: 创建文档体系

**Files:**
- Create: `docs/README.md`（索引）
- Create: `docs/architecture/overview.md`（从 README.md 的 Architecture 段抽出并扩展）
- Create: `docs/guides/setup.md`（安装 + 开发 + hook 配置）
- Create: `docs/plans/README.md`（plan 格式规范）

---

## Task 4: 精简 README.md

把 README.md 里的详细架构、命令列表、数据文件表等移到 docs/ 下。README.md 只保留：
- 一段话介绍
- 安装方式（DMG / 源码）
- 截图（如有）
- 链接到 docs/ 的详细文档

---

## Task 5: 提炼踩坑记录到 AGENTS.md 约束段

从我们这次开发过程中提炼出的关键约束（这些如果没有记录，下次 AI 会重新踩一遍）：

| 坑 | 约束规则 |
|----|---------|
| `tell application "iTerm"` 跳桌面 | arrange 必须用 System Events，不能用 iTerm AppleScript |
| `set frontmost to true` 跳桌面 | 用 `AXRaise` 逐窗口提升 |
| osascript 没有辅助功能权限 | arrange 脚本必须用 NSAppleScript 进程内执行 |
| `primary_monitor()` 可能是另一个屏幕 | 用 `current_monitor()` |
| `$ITERM_SESSION_ID` 带 `wNtNpN:` 前缀 | 进 AppleScript 前 normalize |
| AppleScript `row` 是保留字 | 用 `gridRow` |
| `window.prompt()` 在 Tauri WebView 里无效 | 用就地编辑 input |
| `set current tab` 是只读的 | 用 `tell s to select` |
| Claude Code `Stop` ≠ 会话结束 | Stop = 空闲，SessionEnd = 真正结束 |
| 同一 iTerm pane 切 worktree 会出重复卡 | SessionStart 去重 |
| 重装 app 后辅助功能权限失效 | binary hash 变了要重新授权 |

---

## Verification Checklist

- [ ] `AGENTS.md` 和 `CLAUDE.md` 内容一致
- [ ] 所有 SPEC.md 的模块列表和实际文件对得上
- [ ] `docs/plans/README.md` 包含 plan 格式模板
- [ ] `README.md` 精简后仍包含核心安装步骤
- [ ] 踩坑约束全部在 AGENTS.md §4 里有记录
- [ ] git commit + push
