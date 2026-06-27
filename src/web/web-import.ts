/**
 * 浏览器端 EPUB 文件导入 —— 文件选择 + 拖放
 */
import { bridge, hasNativeBridge } from '../platform'

export function renderWebImport(root: HTMLElement): void {
  if (hasNativeBridge()) return // Tauri 端不渲染此 UI

  const container = document.createElement('div')
  container.className = 'web-import'
  container.innerHTML = `
    <div class="web-import__dropzone" id="web-import-dropzone">
      <p class="web-import__icon">📖</p>
      <p>拖放 EPUB 文件到此处</p>
      <p class="web-import__or">或</p>
      <input type="file" id="web-import-file" accept=".epub" hidden>
      <button id="web-import-btn" class="web-import__btn">选择文件</button>
      <p id="web-import-error" class="web-import__error" hidden></p>
    </div>
  `

  root.appendChild(container)

  const dropzone = container.querySelector('#web-import-dropzone')!
  const fileInput = container.querySelector<HTMLInputElement>('#web-import-file')!
  const btn = container.querySelector('#web-import-btn')!
  const errorEl = container.querySelector<HTMLElement>('#web-import-error')!

  btn.addEventListener('click', () => fileInput.click())

  fileInput.addEventListener('change', () => {
    const file = fileInput.files?.[0]
    if (file) handleFile(file, errorEl)
  })

  dropzone.addEventListener('dragover', (e) => {
    e.preventDefault()
    dropzone.classList.add('web-import__dropzone--active')
  })

  dropzone.addEventListener('dragleave', () => {
    dropzone.classList.remove('web-import__dropzone--active')
  })

  dropzone.addEventListener('drop', (e: Event) => {
    e.preventDefault()
    dropzone.classList.remove('web-import__dropzone--active')
    const de = e as DragEvent
    const file = de.dataTransfer?.files?.[0]
    if (file) handleFile(file, errorEl)
  })
}

async function handleFile(file: File, errorEl: HTMLElement): Promise<void> {
  if (!file.name.endsWith('.epub')) {
    showError(errorEl, '请选择 .epub 格式的文件')
    return
  }
  hideError(errorEl)

  try {
    const buf = await file.arrayBuffer()
    const data = new Uint8Array(buf)
    const info = await bridge.openBookFromBytes(data)
    // 打开成功，移除导入 UI，交给 reader-engine 渲染
    const importEl = document.querySelector('.web-import')
    if (importEl) importEl.remove()

    // 派发自定义事件通知 app 层
    window.dispatchEvent(new CustomEvent('book-opened', { detail: info }))
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err)
    showError(errorEl, `导入失败: ${msg}`)
  }
}

function showError(el: HTMLElement, msg: string): void {
  el.textContent = msg
  el.hidden = false
}

function hideError(el: HTMLElement): void {
  el.hidden = true
}
