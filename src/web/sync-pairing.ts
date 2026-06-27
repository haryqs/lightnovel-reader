/**
 * 同步配对 UI —— 浏览器端输入配对码或服务地址进行设备配对。
 * 桌面端（Tauri）使用此 UI 显示配对码。
 */
import { bridge } from '../platform'

export function renderSyncPairingUI(root: HTMLElement): void {
  const container = document.createElement('div')
  container.className = 'sync-pairing'
  container.innerHTML = `
    <div class="sync-pairing__card" id="sync-pairing-card">
      <h3>设备同步</h3>
      <p class="sync-pairing__desc">将阅读进度、标注和书架跨设备同步</p>

      <div id="sync-pairing-paired" hidden>
        <p class="sync-pairing__status">✅ 已配对</p>
        <p id="sync-pairing-library-id" class="sync-pairing__detail"></p>
        <button id="sync-pairing-unpair" class="sync-pairing__btn sync-pairing__btn--danger">取消配对</button>
      </div>

      <div id="sync-pairing-unpaired">
        <label for="sync-server-url">同步服务器地址</label>
        <input id="sync-server-url" type="text" placeholder="http://your-server:9876" class="sync-pairing__input">
        <label for="sync-pairing-code">配对码（从已配对的设备获取）</label>
        <div class="sync-pairing__row">
          <input id="sync-pairing-code" type="text" placeholder="6位配对码" maxlength="6" class="sync-pairing__input sync-pairing__input--code">
          <button id="sync-pairing-pair" class="sync-pairing__btn">配对</button>
        </div>
        <p id="sync-pairing-error" class="sync-pairing__error" hidden></p>
      </div>
    </div>
  `

  root.appendChild(container)

  const pairedDiv = container.querySelector<HTMLElement>('#sync-pairing-paired')!
  const unpairedDiv = container.querySelector<HTMLElement>('#sync-pairing-unpaired')!
  const libraryIdEl = container.querySelector<HTMLElement>('#sync-pairing-library-id')!
  const errorEl = container.querySelector<HTMLElement>('#sync-pairing-error')!
  const serverInput = container.querySelector<HTMLInputElement>('#sync-server-url')!
  const codeInput = container.querySelector<HTMLInputElement>('#sync-pairing-code')!
  const pairBtn = container.querySelector('#sync-pairing-pair')!
  const unpairBtn = container.querySelector('#sync-pairing-unpair')!

  // Restore saved server URL
  const savedUrl = localStorage.getItem('lnr-sync-server-url')
  if (savedUrl) serverInput.value = savedUrl

  // Check current status
  refreshStatus()

  pairBtn.addEventListener('click', async () => {
    const serverUrl = serverInput.value.trim()
    if (!serverUrl) {
      showError(errorEl, '请输入同步服务器地址')
      return
    }
    const code = codeInput.value.trim()
    if (!code || code.length < 4) {
      showError(errorEl, '请输入配对码')
      return
    }

    hideError(errorEl)
    pairBtn.textContent = '配对中...'
    pairBtn.setAttribute('disabled', 'true')

    try {
      localStorage.setItem('lnr-sync-server-url', serverUrl)
      // Temporarily store server URL for syncPair to use
      localStorage.setItem('lnr-sync-cred', JSON.stringify({ serverUrl }))
      await bridge.syncPair(code)
      refreshStatus()
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      showError(errorEl, `配对失败: ${msg}`)
    } finally {
      pairBtn.textContent = '配对'
      pairBtn.removeAttribute('disabled')
    }
  })

  unpairBtn.addEventListener('click', async () => {
    await bridge.syncUnpair()
    refreshStatus()
  })

  async function refreshStatus(): Promise<void> {
    const status = await bridge.syncStatus()
    if (status.paired) {
      pairedDiv.hidden = false
      unpairedDiv.hidden = true
      libraryIdEl.textContent = `资料库: ${status.libraryId || '未知'}`
    } else {
      pairedDiv.hidden = true
      unpairedDiv.hidden = false
    }
  }
}

function showError(el: HTMLElement, msg: string): void {
  el.textContent = msg
  el.hidden = false
}

function hideError(el: HTMLElement): void {
  el.hidden = true
}
