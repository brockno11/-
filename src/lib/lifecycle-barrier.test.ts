import { afterEach, describe, expect, it, vi } from 'vitest'
import { waitForNoteSaves, type SaveBarrierReady, type SaveBarrierFailure } from './save-barrier'

function channel<T>() {
  let handler: ((payload: T) => void) | undefined
  const unsubscribe = vi.fn(() => { handler = undefined })
  return {
    subscribe: vi.fn(async (next: (payload: T) => void) => { handler = next; return unsubscribe }),
    emit: (payload: T) => handler?.(payload), unsubscribe,
  }
}
afterEach(() => vi.useRealTimers())

describe('save barrier across visible and hidden sessions', () => {
  it('waits for hidden sessions and ignores acknowledgements from an earlier operation', async () => {
    vi.useFakeTimers()
    const ready = channel<SaveBarrierReady>(), failed = channel<SaveBarrierFailure>()
    let completed = false
    const waiting = waitForNoteSaves({
      noteIds: ['visible', 'hidden'], operationId: 'current', operationName: '退出轻笺',
      subscribeReady: ready.subscribe, subscribeFailed: failed.subscribe,
      requestSave: async () => {
        ready.emit({ operationId: 'earlier', noteId: 'hidden' })
        ready.emit({ operationId: 'current', noteId: 'visible' })
      },
    }).then(() => { completed = true })
    await vi.advanceTimersByTimeAsync(100)
    expect(completed).toBe(false)
    ready.emit({ operationId: 'current', noteId: 'hidden' })
    await waiting
    expect(ready.unsubscribe).toHaveBeenCalledOnce()
    expect(failed.unsubscribe).toHaveBeenCalledOnce()
  })
  it('rejects failures without accepting a stale failure', async () => {
    const ready = channel<SaveBarrierReady>(), failed = channel<SaveBarrierFailure>()
    await expect(waitForNoteSaves({
      noteIds: ['a'], operationId: 'current', operationName: '安装更新',
      subscribeReady: ready.subscribe, subscribeFailed: failed.subscribe,
      requestSave: async () => {
        failed.emit({ operationId: 'earlier', noteId: 'a', message: '旧操作' })
        failed.emit({ operationId: 'current', noteId: 'a', message: '磁盘已满' })
      },
    })).rejects.toThrow('便签保存失败（磁盘已满）')
  })
  it('times out a missing session instead of allowing a destructive operation', async () => {
    vi.useFakeTimers()
    const ready = channel<SaveBarrierReady>(), failed = channel<SaveBarrierFailure>()
    const waiting = waitForNoteSaves({
      noteIds: ['a'], operationId: 'current', operationName: '归档', timeoutMs: 2000,
      subscribeReady: ready.subscribe, subscribeFailed: failed.subscribe, requestSave: async () => undefined,
    })
    const assertion = expect(waiting).rejects.toThrow('仍有 1 张便签未完成保存')
    await vi.advanceTimersByTimeAsync(2000)
    await assertion
  })
  it('cleans the first subscription when the second subscription fails', async () => {
    const ready = channel<SaveBarrierReady>()
    await expect(waitForNoteSaves({
      noteIds: ['a'], operationId: 'current', operationName: '备份',
      subscribeReady: ready.subscribe, subscribeFailed: async () => { throw new Error('监听失败') },
      requestSave: async () => undefined,
    })).rejects.toThrow('监听失败')
    expect(ready.unsubscribe).toHaveBeenCalledOnce()
  })
})
