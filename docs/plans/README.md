# Plan 工作流

## 什么时候需要写 plan

- 新功能
- 跨模块改动（Rust + React 联动）
- 架构变更
- 涉及 macOS 系统 API 的改动（AppleScript / Accessibility / CGS）

纯 CSS 修改、文案翻译、单文件 bug fix 不需要。

## Plan 存放路径

```
docs/plans/YYYY-MM-DD/<topic>.md
```

例：`docs/plans/2026-04-13/ai-engineering-practices.md`

## Plan 模板

```markdown
# [标题]

**Goal:** [一句话目标]

**Tech Stack / 影响范围:** [涉及的模块和文件]

---

## 改造概览

[整体设计，可以是文字或 ASCII 图]

---

## Task 1: [任务标题]

**Files:**
- Create: `path/to/new/file`
- Modify: `path/to/existing/file`

- [ ] Step 1: ...
- [ ] Step 2: ...

---

## Task 2: ...

---

## Verification Checklist

- [ ] cargo check 通过
- [ ] tsc --noEmit 通过
- [ ] 手动测试 [具体场景]
- [ ] git commit + push
```

## 工作流

1. **写 plan** → 放到 `docs/plans/YYYY-MM-DD/`
2. **讨论** → 和用户/团队确认方案
3. **实施** → 按 Task 顺序执行
4. **验证** → 勾选 Verification Checklist
5. **提交** → commit 时引用 plan 路径
