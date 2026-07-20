# Plugin Repository Smoke Test 测试限制说明

## 脚本概述

插件仓库有两条互补 smoke：

- `scripts/smoke-plugin-repository-signature.mjs`：无需 GUI/公网，使用临时 Ed25519 密钥串联夹具生成、正式签名工具、
  原始 zip 验签、篡改/错误公钥拒绝，以及 reading-core/Tauri 胶水层测试。
- `scripts/tauri-plugin-repository-smoke.mjs`：WebDriver 下验证本地包预览、安装、启停和卸载；网络仓库仍只验证错误路径。

先运行离线签名 smoke：

```powershell
npm.cmd run smoke:plugin-repository-signature
```

脚本默认删除包含临时私钥的临时目录；仅诊断时可加 `-- --keep-data`，保留目录不得发布。

## 运行要求

### 1. 前置依赖
- 需要先构建调试版应用：`npm run tauri -- build --debug --no-bundle`
- 需要安装 `tauri-driver` 和 Edge WebDriver
- 需要先生成插件仓库 fixtures：`npm run smoke:plugin-repository-fixtures`

### 2. 运行命令
```bash
# 完整测试流程
npm run smoke:plugin-repo

# 跳过 fixture 生成（如已存在）
npm run smoke:plugin-repo -- --skip-fixtures

# 保持应用窗口打开以便调试
npm run smoke:plugin-repo -- --keep-open
```

## 测试范围与限制

### ✅ 测试覆盖范围
1. **插件包检查** (`plugin_inspect_package`)
   - 解析插件 manifest.json
   - 验证插件元数据完整性
   - 检查插件合法性声明
   
2. **插件安装流程** (`plugin_install_package`)
   - 本地 ZIP 包安装
   - 用户合法确认模拟
   - 安装后状态验证
   
3. **插件管理** (`plugin_list_installed`, `plugin_set_enabled`, `plugin_uninstall`)
   - 列出已安装插件
   - 启用/禁用插件
   - 卸载插件

4. **错误处理**
   - 网络不可达仓库 URL 的错误处理
   - 文件权限或路径问题的容错

### ❌ 测试限制

1. **网络仓库测试受限**
   - 当前 WebDriver 脚本没有受信测试 HTTPS 服务，因此只覆盖网络错误处理
   - 离线签名 smoke 已覆盖发布工具与应用验签原语，但不经过真实 TLS/下载器
   - `plugin_load_repository_index` 成功路径和预览/安装两次真实下载仍需受控 HTTPS fixture 或正式仓库验证

2. **插件执行环境隔离**
   - 测试中不执行插件 JavaScript 代码
   - 不验证 QuickJS 运行时集成
   - 只测试 metadata 和安装/卸载机制

3. **平台依赖**
   - 需要真实的 WebDriver 环境
   - 需要构建的 Tauri 应用
   - `msedgedriver` 与 WebView2 Runtime 的前三段版本必须一致；脚本只会选已安装的最高版本，不能保证它自动匹配 Runtime
   - 无法在 CI 无头环境中运行（除非配置了无头浏览器）

4. **并发安装未测试**
   - 只测试单个插件的生命周期
   - 未测试多插件并发安装/卸载
   - 未测试插件冲突处理

5. **数据持久化有限验证**
   - 使用临时数据目录
   - 不测试跨应用重启的插件状态保持

## 预期行为

### 成功场景
- 插件包检查返回完整的 `PluginInstallPreview`
- 安装后插件出现在已安装列表
- 启用/禁用状态正确切换
- 卸载后插件从列表中移除

### 预期失败场景
- 假 HTTPS 仓库 URL 应该触发网络错误
- 路径不存在的插件包应该失败并报告清晰错误

## 调试指南

### 常见问题
1. **应用启动失败**: 检查 `target/debug/reader.exe` 是否存在
2. **WebDriver 连接失败**: 检查 Edge WebDriver 版本兼容性
3. **权限错误**: 确保临时目录有读写权限
4. **插件安装失败**: 检查 `LIGHTNOVEL_READER_APP_DATA_DIR` 权限

### 调试选项
```bash
# 保持窗口打开以便手动检查
npm run smoke:plugin-repo -- --keep-open

# 使用自定义数据目录
npm run smoke:plugin-repo -- --app-data-dir ./debug-plugin-data

# 使用自定义 fixtures 目录
npm run smoke:plugin-repo -- --fixtures-dir ./custom-fixtures
```

## 与其他测试的关系

- **前置**: `smoke:plugin-repository-fixtures` 生成测试用插件包
- **离线签名链**: `smoke:plugin-repository-signature` 自动生成并清理临时密钥/夹具
- **互补**: `tauri-webdriver-smoke.mjs` 测试基础 WebDriver 集成
- **后续**: 真实 HTTPS 签名仓库与正式 keyring 仍需窗口端到端验证
