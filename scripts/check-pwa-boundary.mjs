import { existsSync, readFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const args = process.argv.slice(2)
const errors = []
const viteConfig = readFileSync(resolve(repoRoot, 'vite.config.ts'), 'utf8')
const mainSource = readFileSync(resolve(repoRoot, 'src', 'main.ts'), 'utf8')
const tauriSource = readFileSync(resolve(repoRoot, 'src-tauri', 'src', 'lib.rs'), 'utf8')

if (!/injectRegister\s*:\s*null/.test(viteConfig)) {
  errors.push('vite-plugin-pwa must not inject an unconditional service worker registration')
}
if (!/if\s*\(isTauriRuntime\(\)\)[\s\S]*?getRegistrations\(\)[\s\S]*?unregister\(\)/.test(mainSource)) {
  errors.push('Tauri runtime must unregister stale PWA service workers')
}
if (!/if\s*\(isTauriRuntime\(\)\)[\s\S]*?caches\.keys\(\)[\s\S]*?caches\.delete/.test(mainSource)) {
  errors.push('Tauri runtime must clear stale PWA CacheStorage entries')
}
if (!/navigator\.serviceWorker\.register\('\/sw\.js'/.test(mainSource)) {
  errors.push('Web runtime must retain explicit PWA service worker registration')
}
const nativeCleanupCall = tauriSource.indexOf('clear_legacy_desktop_pwa_cache(&context.config().identifier);')
const tauriBuilder = tauriSource.indexOf('tauri::Builder::default()')
if (nativeCleanupCall < 0 || tauriBuilder < 0 || nativeCleanupCall > tauriBuilder) {
  errors.push('Windows legacy PWA cache cleanup must run before Tauri creates WebView2')
}
if (!/\["Cache", "Code Cache", "Service Worker"\]/.test(tauriSource)) {
  errors.push('Windows legacy PWA cleanup must remove HTTP, code and service worker cache only')
}

if (args.includes('--dist')) {
  const distRoot = resolve(repoRoot, 'dist')
  const indexPath = resolve(distRoot, 'index.html')
  const serviceWorkerPath = resolve(distRoot, 'sw.js')
  if (!existsSync(indexPath)) errors.push('dist/index.html is missing')
  if (!existsSync(serviceWorkerPath)) errors.push('dist/sw.js is missing for Web/PWA')
  if (existsSync(indexPath)) {
    const builtIndex = readFileSync(indexPath, 'utf8')
    if (/registerSW\.js|vite-plugin-pwa:register-sw/.test(builtIndex)) {
      errors.push('dist/index.html still injects service worker registration unconditionally')
    }
  }
  if (existsSync(resolve(distRoot, 'registerSW.js'))) {
    errors.push('dist/registerSW.js must not be generated')
  }
}

if (errors.length > 0) {
  console.error('check-pwa-boundary: BLOCKED')
  for (const error of errors) console.error(`- ${error}`)
  process.exitCode = 1
} else {
  console.log(`check-pwa-boundary: OK(dist=${args.includes('--dist') ? 'verified' : 'skipped'})`)
}
