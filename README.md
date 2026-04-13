# AgentManager

macOS Claude Code 会话管理器。左侧常驻面板实时展示 Claude 会话状态，一键跳转 iTerm、排列窗口、恢复历史会话。

![macOS](https://img.shields.io/badge/macOS-12.0%2B-blue) ![License](https://img.shields.io/badge/license-MIT-green)

**Tauri 2 + Rust + React 18 + TypeScript**

## 功能

- **实时会话卡片** — SessionStart/Stop/Notification/SessionEnd 五个 hook，Claude 启动即可见
- **点击跳转** — 聚焦到对应 iTerm 窗口/Tab/Split Pane
- **排列窗口** — 一键把所有 iTerm 窗口排成网格（当前桌面，不跳 Space）
- **重命名** — 卡片 + Claude 历史都可重命名，搜索可匹配
- **Claude 历史** — 扫描 `~/.claude/` 全量历史会话，点击恢复（`claude --resume`）
- **会话持久化** — 重启不丢，最多 200 条
- **中英切换** — 🌐 按钮一键切换

## 安装

### 下载 DMG

[**Releases**](https://github.com/MuXiaoCao/AgentManager/releases/latest) → 下载 DMG → 拖到 Applications → 右键打开

### 源码构建

```bash
git clone https://github.com/MuXiaoCao/AgentManager.git
cd AgentManager
npm install
npm run tauri:build
cp -R src-tauri/target/release/bundle/macos/AgentManager.app /Applications/
```

打开后点顶部蓝色横幅 **"一键安装"** 配置 Claude Code hook，然后重启正在运行的 Claude 会话。

## 文档

| 文档 | 内容 |
|------|------|
| [安装与开发指南](docs/guides/setup.md) | 详细安装、开发、hook 配置 |
| [系统架构](docs/architecture/overview.md) | 数据流、设计决策、技术栈 |
| [AI 工作协议](AGENTS.md) | Agent harness、约束、plan 工作流 |
| [Rust 模块 SPEC](src-tauri/SPEC.md) | 后端文件职责、关键约束 |
| [React 模块 SPEC](src/SPEC.md) | 前端组件树、状态管理 |
| [Plan 规范](docs/plans/README.md) | 实施计划格式和工作流 |

## License

MIT
