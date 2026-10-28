export type SaveBarrierReady = {
  operationId: string
  noteId: string
}
export type SaveBarrierFailure = SaveBarrierReady & {
  message: string
}
export type SaveBarrierRequest = {
  operationId: string
  operationName: string
  noteIds: string[]
}

type Unlisten = () => void
type Subscribe<T> = (handler: (payload: T) => void) => Promise<Unlisten>

type SaveBarrierOptions = {
  noteIds: Iterable<string>
  operationId: string
  operationName: string
  timeoutMs?: number
  subscribeReady: Subscribe<SaveBarrierReady>
  subscribeFailed: Subscribe<SaveBarrierFailure>
  requestSave: () => Promise<void>
}

/**
 * Waits until every live editing session (including hidden windows) confirms
 * this operation's save. Late replies from a previous operation are ignored.
 * A failure or timeout rejects the operation; neither is treated as permission
 * to quit or install an update.
 */
export async function waitForNoteSaves({
  noteIds,
  operationId,
  operationName,
  timeoutMs = 10_000,
  subscribeReady,
  subscribeFailed,
  requestSave,
}: SaveBarrierOptions): Promise<void> {
  const pending = new Set(noteIds)
  if (!pending.size) return

  let settled = false
  let resolveCompletion!: () => void
  let rejectCompletion!: (reason: Error) => void
  const completion = new Promise<void>((resolve, reject) => {
    resolveCompletion = resolve
    rejectCompletion = reject
  })

  const markReady = (reply: SaveBarrierReady) => {
    if (reply.operationId !== operationId || settled || !pending.delete(reply.noteId)) return
    if (!pending.size) {
      settled = true
      resolveCompletion()
    }
  }

  const markFailed = ({ operationId: replyId, noteId, message }: SaveBarrierFailure) => {
    if (replyId !== operationId || settled || !pending.has(noteId)) return
    settled = true
    rejectCompletion(new Error(`无法${operationName}：便签保存失败（${message}）。请确认磁盘空间和文件权限后重试。`))
  }

  const unlisten: Unlisten[] = []
  let timeout: ReturnType<typeof setTimeout> | undefined

  try {
    unlisten.push(await subscribeReady(markReady))
    unlisten.push(await subscribeFailed(markFailed))
    timeout = setTimeout(() => {
      if (settled) return
      settled = true
      rejectCompletion(new Error(`无法${operationName}：仍有 ${pending.size} 张便签未完成保存。请稍后重试。`))
    }, timeoutMs)
    // A failed save may arrive while the IPC request is still in flight.
    // Attach the rejection handler before dispatching to avoid unhandled errors.
    await Promise.all([completion, requestSave()])
  } finally {
    clearTimeout(timeout)
    unlisten.forEach((stop) => stop())
  }
}
