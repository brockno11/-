import { describe, expect, it, vi } from 'vitest'
import { SaveCoordinator } from './save-coordinator'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

describe('SaveCoordinator', () => {
  it('serializes saves and preserves edits queued during an older request', async () => {
    const first = deferred<string>()
    const second = deferred<string>()
    const save = vi.fn()
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise)
    const states: string[] = []
    const saved: string[] = []
    const coordinator = new SaveCoordinator<string, string>({
      save,
      onStateChange: (state) => states.push(state),
      onSaved: (output, _revision, isLatest) => { if (isLatest) saved.push(output) },
    })

    coordinator.queue('第一版')
    const flushing = coordinator.flush()
    coordinator.queue('第二版')
    first.resolve('第一版')
    await Promise.resolve()
    expect(save).toHaveBeenCalledTimes(2)
    expect(save).toHaveBeenLastCalledWith('第二版')
    second.resolve('第二版')
    await flushing

    expect(saved).toEqual(['第二版'])
    expect(coordinator.hasPendingChanges).toBe(false)
    expect(states.at(-1)).toBe('saved')
  })

  it('keeps a failed revision pending so retry can save it', async () => {
    const save = vi.fn().mockRejectedValueOnce(new Error('磁盘繁忙')).mockResolvedValueOnce('已保存')
    const states: string[] = []
    const coordinator = new SaveCoordinator<string, string>({
      save,
      onStateChange: (state) => states.push(state),
    })
    coordinator.queue('不能丢失')
    await expect(coordinator.flush()).rejects.toThrow('磁盘繁忙')
    expect(coordinator.hasPendingChanges).toBe(true)
    await coordinator.flush()
    expect(save).toHaveBeenCalledTimes(2)
    expect(coordinator.hasPendingChanges).toBe(false)
    expect(states.at(-1)).toBe('saved')
  })
})
