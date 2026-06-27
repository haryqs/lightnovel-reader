/**
 * 网页端存储层 —— IndexedDB（元数据/标注/进度）+ OPFS（EPUB 文件）
 *
 * 在不支持 OPFS 的浏览器自动降级为全 IndexedDB。
 */

const DB_NAME = 'lightnovel-reader-web'
const DB_VERSION = 1

interface BookRecord {
  bookId: string
  title: string
  author?: string
  language?: string
  description?: string
  series?: string
  seriesIndex?: number
  tocJson: string
  spineJson: string
  addedAt: number
  lastReadAt: number
}

interface AnnotationRecord {
  id: string
  bookId: string
  cfi: string
  text: string
  note: string
  color: string
  createdAt: number
}

interface ProgressRecord {
  bookId: string
  cfi: string
  percentage: number
  updatedAt: number
}

// ---- DB 初始化 ----

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION)
    req.onupgradeneeded = () => {
      const db = req.result
      if (!db.objectStoreNames.contains('books')) {
        db.createObjectStore('books', { keyPath: 'bookId' })
      }
      if (!db.objectStoreNames.contains('annotations')) {
        const store = db.createObjectStore('annotations', { keyPath: 'id' })
        store.createIndex('bookId', 'bookId', { unique: false })
      }
      if (!db.objectStoreNames.contains('progress')) {
        db.createObjectStore('progress', { keyPath: 'bookId' })
      }
    }
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

function idbReq<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}

// ---- 书目 ----

export async function saveBook(record: BookRecord): Promise<void> {
  const db = await openDB()
  await idbReq(db.transaction('books', 'readwrite').objectStore('books').put(record))
}

export async function listBooks(): Promise<BookRecord[]> {
  const db = await openDB()
  return idbReq(db.transaction('books', 'readonly').objectStore('books').getAll())
}

export async function getBook(bookId: string): Promise<BookRecord | undefined> {
  const db = await openDB()
  return idbReq(db.transaction('books', 'readonly').objectStore('books').get(bookId))
}

export async function removeBook(bookId: string): Promise<void> {
  const db = await openDB()
  await idbReq(db.transaction('books', 'readwrite').objectStore('books').delete(bookId))
}

// ---- 标注 ----

export async function saveAnnotation(ann: AnnotationRecord): Promise<void> {
  const db = await openDB()
  await idbReq(db.transaction('annotations', 'readwrite').objectStore('annotations').put(ann))
}

export async function listAnnotations(bookId: string): Promise<AnnotationRecord[]> {
  const db = await openDB()
  const store = db.transaction('annotations', 'readonly').objectStore('annotations')
  return idbReq(store.index('bookId').getAll(bookId))
}

export async function deleteAnnotation(id: string): Promise<void> {
  const db = await openDB()
  await idbReq(db.transaction('annotations', 'readwrite').objectStore('annotations').delete(id))
}

// ---- 阅读进度 ----

export async function saveProgress(progress: ProgressRecord): Promise<void> {
  const db = await openDB()
  await idbReq(db.transaction('progress', 'readwrite').objectStore('progress').put(progress))
}

export async function getProgress(bookId: string): Promise<ProgressRecord | undefined> {
  const db = await openDB()
  return idbReq(db.transaction('progress', 'readonly').objectStore('progress').get(bookId))
}

// ---- OPFS（EPUB 文件本体）----

let _opfsRoot: FileSystemDirectoryHandle | null = null

async function opfsRoot(): Promise<FileSystemDirectoryHandle | null> {
  if (_opfsRoot !== null) return _opfsRoot
  try {
    _opfsRoot = await navigator.storage.getDirectory()
    return _opfsRoot
  } catch {
    _opfsRoot = null
    return null
  }
}

/** 保存 EPUB 文件到 OPFS，失败降级到 IndexedDB */
export async function saveEpubFile(bookId: string, data: ArrayBuffer): Promise<void> {
  const root = await opfsRoot()
  if (root) {
    const fileHandle = await root.getFileHandle(bookId, { create: true })
    const writable = await fileHandle.createWritable()
    await writable.write(data)
    await writable.close()
  } else {
    // 降级：IndexedDB
    const db = await openDB()
    const tx = db.transaction('books', 'readwrite')
    const store = tx.objectStore('books')
    const record = await idbReq(store.get(bookId))
    if (record) {
      ;(record as unknown as Record<string, unknown>)._epubData = data
      await idbReq(store.put(record))
    }
  }
}

/** 从 OPFS 读取 EPUB 文件，失败降级到 IndexedDB */
export async function loadEpubFile(bookId: string): Promise<ArrayBuffer | null> {
  const root = await opfsRoot()
  if (root) {
    try {
      const fileHandle = await root.getFileHandle(bookId)
      const file = await fileHandle.getFile()
      return file.arrayBuffer()
    } catch {
      return null
    }
  }
  // 降级：IndexedDB
  const db = await openDB()
  const record = await idbReq(db.transaction('books', 'readonly').objectStore('books').get(bookId))
  if (record) {
    const data = (record as unknown as Record<string, unknown>)._epubData
    if (data instanceof ArrayBuffer) return data
  }
  return null
}

/** 删除 EPUB 文件 */
export async function removeEpubFile(bookId: string): Promise<void> {
  const root = await opfsRoot()
  if (root) {
    try {
      await root.removeEntry(bookId)
    } catch {
      // not found, ignore
    }
  }
}
