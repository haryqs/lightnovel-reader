#!/usr/bin/env node
/**
 * 冷启动计时脚本 —— 测量 Tauri 应用启动到首屏可交互的时间。
 *
 * 用法（需先构建 Release 版）：
 *   npm run tauri build
 *   node scripts/measure-cold-start.mjs [path-to-exe]
 *
 * 默认路径：
 *   src-tauri/target/release/reader.exe
 */
const { execSync } = require('child_process')
const { performance } = require('perf_hooks')
const path = require('path')
const fs = require('fs')

const exePath = process.argv[2] || path.join(__dirname, '..', 'src-tauri', 'target', 'release', 'reader.exe')

if (!fs.existsSync(exePath)) {
  console.error(`未找到 exe: ${exePath}`)
  console.error('请先运行: npm run tauri build')
  console.error('或指定路径: node scripts/measure-cold-start.mjs <path-to-exe>')
  process.exit(1)
}

const ITERATIONS = 3
const results = []

for (let i = 0; i < ITERATIONS; i++) {
  console.log(`第 ${i + 1}/${ITERATIONS} 次冷启动...`)

  // 确保前一个实例完全退出
  try { execSync('taskkill /f /im reader.exe 2>nul', { stdio: 'ignore' }) } catch {}
  // 等待 500ms 确保进程完全退出
  execSync('timeout /t 1 /nobreak >nul', { stdio: 'ignore' })

  const start = performance.now()
  try {
    // 启动应用，等待 3 秒后杀进程
    const child = require('child_process').spawn(exePath, [], { detached: true, stdio: 'ignore' })
    // 等待进程启动（webview 初始化需要时间）
    await new Promise(resolve => setTimeout(resolve, 3000))
    // 检查进程是否还活着
    try {
      process.kill(child.pid, 0) // 信号 0 = 检查进程是否存在
      const elapsed = (performance.now() - start - 3000).toFixed(0) // 减去等待时间
      console.log(`  进程存活，估计启动 < 3000ms`)
      results.push(parseInt(elapsed))
      // 杀进程
      try { execSync(`taskkill /f /pid ${child.pid} 2>nul`, { stdio: 'ignore' }) } catch {}
    } catch {
      console.log(`  进程已退出（可能启动失败）`)
      results.push(9999)
    }
  } catch (err) {
    console.log(`  启动失败: ${err.message}`)
    results.push(9999)
  }
}

if (results.filter(r => r < 9000).length > 0) {
  const valid = results.filter(r => r < 9000)
  const avg = valid.reduce((a, b) => a + b, 0) / valid.length
  console.log(`\n--- 结果 ---`)
  console.log(`有效测试: ${valid.length}/${ITERATIONS}`)
  console.log(`目标: < 1000ms`)
  console.log(`评估: 进程在 3s 内启动，但精确冷启动时间需用 Tauri API 测量（window.show 时间戳）`)
  console.log(`建议: 在 lib.rs setup 中打印 startup_ms，或用 DevTools Performance 录制`)
} else {
  console.log(`\n--- 结果 ---`)
  console.log(`所有测试失败，请确认 Release 构建成功`)
  console.log(`运行: cd src-tauri && cargo build --release`)
}
