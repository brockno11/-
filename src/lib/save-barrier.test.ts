import { afterEach, describe, expect, it, vi } from 'vitest'
import { waitForNoteSaves, type SaveBarrierReady, type SaveBarrierFailure } from './save-barrier'

function channel<T>() {
  let handler: ((payload: T) => void) | undefined
  return {
    subscribe: vi.fn(async (next: (payload: T) => void) => {
      handler = next
      return () => { handler = undefined }
    }),
    emit: (payload: T) => handler?.(payload),
  }
}

afterEach(() => vi.useRealTimers())

describe('waitForNoteSaves', () => {
  it('continues only after every requested note reports a successful save', async () => {
    const ready = channel<SaveBarrierReady>()
    const failed = channel<SaveBarrierFailure>()
    const requestSave = vi.fn(async () => {
      ready.emit({ operationId: 'test', noteId: 'note-a' })
      ready.emit({ operationId: 'test', noteId: 'note-b' })
    })

    await waitForNoteSaves({
      noteIds: ['note-a', 'note-b'],
      operationId: 'test', operationName: '退出轻笺',
      subscribeReady: ready.subscribe,
      subscribeFailed: failed.subscribe,
      requestSave,
    })

    expect(requestSave).toHaveBeenCalledOnce()
  })

  it('rejects immediately when any note reports a save failure', async () => {
    const ready = channel<SaveBarrierReady>()
    const failed = channel<SaveBarrierFailure>()

    await expect(waitForNoteSaves({
      noteIds: ['note-a'],
      operationId: 'test', operationName: '安装更新',
      subscribeReady: ready.subscribe,
      subscribeFailed: failed.subscribe,
      requestSave: async () => failed.emit({ operationId: 'test', noteId: 'note-a', message: '磁盘已满' }),
    })).rejects.toThrow('无法安装更新：便签保存失败（磁盘已满）')
  })

  it('rejects a slow save instead of treating the timeout as success', async () => {
    vi.useFakeTimers()
    const ready = channel<SaveBarrierReady>()
    const failed = channel<SaveBarrierFailure>()
    const waiting = waitForNoteSaves({
      noteIds: ['note-a'],
      operationId: 'test', operationName: '退出轻笺',
      timeoutMs: 2_000,
      subscribeReady: ready.subscribe,
      subscribeFailed: failed.subscribe,
      requestSave: async () => undefined,
    })
    const assertion = expect(waiting).rejects.toThrow('仍有 1 张便签未完成保存')

    await vi.advanceTimersByTimeAsync(2_000)
    await assertion
  })
})
