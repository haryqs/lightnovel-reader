# 工具、Skill 与插件候选

> 这里记录未来要安装或启用的能力。外部下载受网络与权限影响时，以仓库内文档流程兜底。

## 已可用能力

- Browser 插件：用于本地 Web/Tauri 前端视觉和交互验证。
- Chrome 插件：需要用户现有登录态时才使用。
- Documents / Spreadsheets / Presentations：处理文档、表格、演示材料。
- Multi-agent：仅在用户明确要求多代理/并行代理时使用。
- Automation：可创建定期文档审计、提醒、监控任务。
- 全局 skill：`define-goal`。
- 全局 skill：`playwright`、`screenshot`。
- 全局 skill：`security-best-practices`、`security-threat-model`、`security-ownership-map`。
- 全局 skill：`notion-knowledge-capture`、`notion-research-documentation`、`notion-spec-to-implementation`。

## 仍可继续寻找的 skill 类型

后续可以继续寻找这些方向：

1. 项目记忆 / memory / knowledge base。
2. 代码审查 / review / static analysis。
3. 文档维护 / docs / changelog。
4. Rust 开发辅助。
5. TypeScript / frontend 开发辅助。
6. 测试策略 / QA。
7. 架构决策记录 / ADR。

## 安装方式

使用系统 `skill-installer`：

```powershell
python C:/Users/41267/.codex/skills/.system/skill-installer/scripts/list-skills.py
```

安装示例：

```powershell
python C:/Users/41267/.codex/skills/.system/skill-installer/scripts/install-skill-from-github.py --repo openai/skills --path skills/.curated/<skill-name>
```

安装后需要重启 Codex 才能生效。

## 已安装记录

2026-06-12 已从 `openai/skills` curated 包安装：

```text
define-goal
playwright
screenshot
security-best-practices
security-threat-model
security-ownership-map
notion-knowledge-capture
notion-research-documentation
notion-spec-to-implementation
```

说明：

- GitHub API 曾因出口 IP rate limit 返回 403，最终改用 codeload zip 下载。
- `gh-address-comments` / `gh-fix-ci` 暂未安装，因为本机未安装 GitHub CLI。
- 安装后需要重启 Codex 才能在新会话自动发现这些全局 skills。

## 建议的自动化

可以后续创建一个每周文档审计 automation：

```text
检查 docs/dev-memory、docs/current-project、docs/resource-library-plan 是否与代码状态一致；
发现过期项时提出修改建议，不直接大改代码。
```
