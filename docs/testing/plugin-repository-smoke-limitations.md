# Plugin Repository Smoke Test 测试限制说明

## 脚本概述

`scripts/tauri-plugin-repository-smoke.mjs` 是插件仓库安装流程的 WebDriver smoke 测试，模拟用户在 lightnovel-reader 中安装插件的完整流程。

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
   - 由于 `ensure_https_plugin_url` 强制 HTTPS，无法测试真实的网络仓库加载
   - 只能测试网络错误的错误处理逻辑
   - 无法验证 `plugin_load_repository_index` 的成功路径

2. **插件执行环境隔离**
   - 测试中不执行插件 JavaScript 代码
   - 不验证 QuickJS 运行时集成
   - 只测试 metadata 和安装/卸载机制

3. **平台依赖**
   - 需要真实的 WebDriver 环境
   - 需要构建的 Tauri 应用
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
- **互补**: `tauri-webdriver-smoke.mjs` 测试基础 WebDriver 集成
- **后续**: 真实插件执行需要额外的集成测试