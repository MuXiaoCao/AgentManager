# src/ — React 前端 SPEC

## 组件树

```
App.tsx
├── header: Tab 栏（面板 / Claude 历史）+ 排列按钮 + 语言切换
├── SetupBanner          — hook 未安装时的蓝色提示条
├── [tab: dashboard]
│   ├── SessionCard[]    — 活跃会话卡片（click=jump, 右键=菜单）
│   └── SessionCard[]    — 历史会话（ended，灰色半透明）
├── [tab: claude-history]
│   └── ClaudeHistoryList — 扫描 ~/.claude/ 的全量会话列表
├── ContextMenu          — 右键弹出菜单（通用组件）
└── Toast                — 底部浮动提示
```

## 文件职责

| 文件 | 职责 |
|------|------|
| `App.tsx` | 主入口：tab 状态、session 列表、所有 action handler |
| `components/SessionCard.tsx` | 单张会话卡片：状态 badge、就地重命名、相对时间 |
| `components/ClaudeHistoryList.tsx` | Claude 历史 tab：磁盘扫描、搜索、右键重命名/恢复 |
| `components/ContextMenu.tsx` | 通用右键菜单（MenuItem[] → 绝对定位浮层） |
| `components/SetupBanner.tsx` | hook 安装引导横幅 |
| `i18n.ts` | react-i18next 初始化 + 语言切换 |
| `locales/en.json` / `zh.json` | 全量字符串目录 |
| `types.ts` | TypeScript 接口定义（SessionEntry / ClaudeHistoryEntry / etc.） |
| `styles.css` | 全局深色主题样式 |

## 状态管理

- **无状态管理库**：纯 `useState` + `useCallback`
- **数据刷新**：`listen("session-updated")` 触发 `invoke("get_sessions")`，无轮询
- **选中态**：`selectedId: string | null`，click/右键/jump 都设
- **重命名态**：`renamingId: string | null`，SessionCard 内渲染 `<input>`
- **Tab 态**：`tab: "dashboard" | "claude-history"`
- **语言**：`localStorage["agent-manager:lang"]` + `i18n.changeLanguage()`

## 事件 → 状态 → 显示 映射

| event_type | tone | 中文 | 颜色 |
|---|---|---|---|
| `sessionstart` | active | 运行中 | 🟢 |
| `userpromptsubmit` | active | 处理中 | 🟢 |
| `stop` | active | 空闲 | 🟢 |
| `notification` | idle | 等待输入 | 🟡 |
| `sessionend` | done | 已结束 | ⚪ |

## 关键约束

1. **不能用 `window.prompt()` / `window.alert()`**：Tauri 2 WebView 屏蔽。用 inline `<input>` 替代。
2. **`notification_count` 在非 notification 事件时清零**：否则 badge 永远递增。
3. **搜索要匹配 alias**：ClaudeHistoryList 的 filter 包含 alias 字段。
