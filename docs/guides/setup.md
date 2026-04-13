# 安装与开发指南

## 用户安装

### 方式 A: 下载 DMG

1. [Releases](https://github.com/MuXiaoCao/AgentManager/releases/latest) 下载 DMG
2. 打开 DMG，拖 `AgentManager.app` 到 `/Applications`
3. 首次启动：右键 → 打开 → 弹窗里点"打开"（Gatekeeper）
4. 点顶部蓝色横幅 **"一键安装"** 配置 Claude Code hook
5. **重启正在运行的 Claude 会话**

### 方式 B: 源码构建

```bash
git clone https://github.com/MuXiaoCao/AgentManager.git
cd AgentManager
npm install
npm run tauri:build
cp -R src-tauri/target/release/bundle/macos/AgentManager.app /Applications/
```

### 前置条件

- macOS 12.0+（Apple Silicon 或 Intel）
- iTerm2
- Claude Code CLI
- `jq`（`brew install jq`）
- 辅助功能权限（首次排列时会弹出授权对话框）

## 开发

```bash
npm install                # 安装依赖
npm run tauri:dev          # 开发模式（HMR）
npx tsc --noEmit           # 类型检查
cargo test --lib           # Rust 单元测试
npm run tauri:build        # Release 构建
```

### 构建后的 post-build 步骤

`npm run tauri:build` 会自动执行 `scripts/patch-plist.sh`，它：
1. 给 Info.plist 补 `NSAppleEventsUsageDescription` key
2. 编译 `scripts/move-to-space.swift` 到 .app bundle（历史遗留，macOS 15 上无效但保留）

### 安装到 /Applications

```bash
pkill -f agent-manager
rm -rf /Applications/AgentManager.app
cp -R src-tauri/target/release/bundle/macos/AgentManager.app /Applications/
open /Applications/AgentManager.app
```

### 辅助功能权限

每次重新安装 app（binary hash 变了），macOS 会撤销辅助功能授权。需要：

```bash
tccutil reset Accessibility com.xiaocao.agentmanager
open /Applications/AgentManager.app
# 点"排列"会弹出授权对话框 → 授权
```

## Hook 配置

AgentManager 安装 5 个 Claude Code hook：

| Hook | 触发时机 |
|------|----------|
| `SessionStart` | Claude 会话启动 |
| `UserPromptSubmit` | 用户发 prompt |
| `Stop` | Claude 一轮回答结束 |
| `Notification` | Claude 需要用户注意 |
| `SessionEnd` | 会话退出 |

所有 hook 指向 `~/.claude-dashboard/hook.sh`，curl 到 `127.0.0.1:19280/api/notify`。

安装方式：AgentManager 内点 **"一键安装"** 横幅（幂等，不影响已有 hook）。
