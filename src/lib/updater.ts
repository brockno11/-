import { getVersion } from '@tauri-apps/api/app'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type DownloadEvent, type Update } from '@tauri-apps/plugin-updater'

const isDesktop = '__TAURI_INTERNALS__' in window
const FALLBACK_VERSION = '0.8.0'

let pendingUpdate: Update | null = null

export interface AppUpdateInfo {
  currentVersion: string
  version: string
  date?: string
  body?: string
}

export interface UpdateDownloadProgress {
  downloadedBytes: number
  totalBytes?: number
  finished: boolean
}

export async function getCurrentAppVersion() {
  return isDesktop ? getVersion() : FALLBACK_VERSION
}

export async function checkForAppUpdate(): Promise<AppUpdateInfo | null> {
  if (!isDesktop) return null
  if (pendingUpdate) {
    await pendingUpdate.close().catch(() => undefined)
    pendingUpdate = null
  }
  const update = await check({ timeout: 15_000 })
  if (!update) return null
  pendingUpdate = update
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date,
    body: update.body,
  }
}

export async function downloadAndInstallAppUpdate(
  onProgress: (progress: UpdateDownloadProgress) => void,
) {
  if (!pendingUpdate) throw new Error('更新信息已失效，请重新检查更新')
  let downloadedBytes = 0
  let totalBytes: number | undefined
  const handleEvent = (event: DownloadEvent) => {
    if (event.event === 'Started') {
      totalBytes = event.data.contentLength
      downloadedBytes = 0
    } else if (event.event === 'Progress') {
      downloadedBytes += event.data.chunkLength
    }
    onProgress({
      downloadedBytes,
      totalBytes,
      finished: event.event === 'Finished',
    })
  }
  await pendingUpdate.downloadAndInstall(handleEvent, { timeout: 120_000 })
  await relaunch()
}
