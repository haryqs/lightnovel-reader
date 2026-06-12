# 开发记忆系统

这个目录用于把“项目记忆”固定在仓库中，避免每次换电脑、换 AI、换线程都重新解释。

## 文件说明

- `PROJECT_MEMORY.md`：项目长期记忆，记录目标、边界、架构纪律。
- `DEVELOPMENT_OUTLINE.md`：从 v0.3 到 v1.0 的开发大纲。
- `DECISIONS.md`：决策日志，记录为什么这么做。
- `DEV_LOG.md`：开发日志，每次重要改动后追加。
- `NEXT_ACTIONS.md`：下一步任务队列。
- `SESSION_TEMPLATE.md`：开工/收工模板。
- `TOOLING_BACKLOG.md`：未来可安装的 skill、插件、工具候选。

## 使用方式

开工：

```powershell
git status -sb
node scripts/check-dev-memory.mjs
```

收工：

```powershell
node scripts/check-arch.mjs
node scripts/check-dev-memory.mjs
```

然后更新 `DEV_LOG.md` 和 `NEXT_ACTIONS.md`。

## 原则

记忆只记录会影响未来开发判断的事实，不记录流水账。
