# HTTP 采集服务自恢复与健康提示

**Goal:** AgentManager 的本地通知服务异常退出后自动恢复，并在服务不可用时向用户展示明确状态，避免 Claude/Codex hook 静默丢失。

**Tech Stack / 影响范围:** Rust/Tauri 后端 `http_server.rs`、IPC 命令与 React 状态提示、双语文案。

---

## 改造概览

HTTP 服务由一次性后台任务改为带退避的常驻监督循环。监听或服务异常时记录错误、更新共享健康状态并自动重试；前端通过 IPC 定期读取状态，在服务不可用时展示提示。

```text
hook -> 127.0.0.1:19280 -> axum
                              |
                    success: healthy
                    failure: unhealthy -> delay -> rebind
                              |
                         Tauri IPC -> React banner
```

---

## Task 1: 后端服务自恢复与状态

**Files:**

- Modify: `src-tauri/src/http_server.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands.rs`

- [x] 增加线程安全的 HTTP 服务状态，包含健康标记、重试次数和最近错误。
- [x] 监听失败或 serve 异常退出后延迟重试，成功监听后恢复健康状态。
- [x] 增加只读 Tauri IPC 命令供前端查询状态。
- [x] 为状态转换补充 Rust 单元测试。

---

## Task 2: 前端健康提示

**Files:**

- Modify: `src/types.ts`
- Modify: `src/App.tsx`
- Modify: `src/locales/en.json`
- Modify: `src/locales/zh.json`

- [x] 启动时及固定间隔查询 HTTP 服务状态。
- [x] 服务不可用时显示非阻塞提示，恢复后自动消失。

---

## Task 3: 验证与文档收尾

**Files:**

- Modify: `docs/plans/2026-07-20/http-server-self-healing.md`

- [x] `cargo test --lib` 通过。
- [x] `npx tsc --noEmit` 通过。
- [x] 验证服务状态序列化和前端状态判断。
- [x] 勾选已完成的 checklist；不自动 commit/push。

---

## Verification Checklist

- [x] `cargo test --lib` 通过
- [x] `npx tsc --noEmit` 通过
- [x] HTTP 服务失败后会持续重试而非永久退出
- [x] 前端可显示服务异常并在恢复后隐藏
- [x] 未改写现有 hook 配置和用户的其他 Codex hooks
