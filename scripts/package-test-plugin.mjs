#!/usr/bin/env node
/**
 * 打包测试插件为 .zip（供 plugin_inspect_package / plugin_install_package 使用）。
 *
 * 用法：node scripts/package-test-plugin.mjs [out-dir]
 */
import { execSync } from 'child_process'
import path from 'path'
import fs from 'fs'
import { fileURLToPath } from 'url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const pluginDir = path.join(__dirname, 'test-plugin')
const outDir = process.argv[2] || pluginDir
const outFile = path.join(outDir, 'test-plugin-hello.zip')

fs.mkdirSync(outDir, { recursive: true })

console.log(`打包 ${pluginDir} → ${outFile}`)
execSync(`powershell -Command "Compress-Archive -Path '${pluginDir}\\manifest.json','${pluginDir}\\index.js' -DestinationPath '${outFile}' -Force"`, { stdio: 'inherit' })

console.log(`✓ 测试插件已打包: ${outFile}`)
console.log(`\n在 Tauri 应用中安装:`)
console.log(`  1. 打开源插件面板 → 选择安装包 → ${outFile}`)
console.log(`  2. 安装后点击"测试"按钮验证 QuickJS 运行时`)
