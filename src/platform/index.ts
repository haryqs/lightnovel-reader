// platform 适配层:reader-engine 与平台壳之间的唯一边界(方案文档 7 的纪律 1)。
// 引擎代码只允许 import 本目录,不允许直接触碰 @tauri-apps/* 或其他平台 API。
import type { ReaderBridge } from './protocol'
import { isTauriRuntime, tauriBridge } from './tauri'
import { webBridge } from '../web/web-bridge'

export * from './protocol'

/** 当前是否运行在带 reading-core 的原生壳里(浏览器直开 vite dev 时为 false)。 */
export const hasNativeBridge = isTauriRuntime

export const bridge: ReaderBridge = isTauriRuntime() ? tauriBridge : webBridge
