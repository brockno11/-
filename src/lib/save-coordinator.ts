export type CoordinatedSaveState = 'saved' | 'saving' | 'dirty' | 'error'

type SaveCoordinatorOptions<Input, Output> = {
  delay?: number
  save: (input: Input) => Promise<Output>
  onStateChange?: (state: CoordinatedSaveState, error?: unknown) => void
  onSaved?: (output: Output, revision: number, isLatest: boolean) => void
}

/**
 * Serializes autosaves and tracks the newest queued revision.
 * A completed older request can never mark newer edits as saved or discard them.
 */
export class SaveCoordinator<Input, Output> {
  private readonly delay: number
  private readonly saveInput: (input: Input) => Promise<Output>
  private readonly onStateChange?: SaveCoordinatorOptions<Input, Output>['onStateChange']
  private readonly onSaved?: SaveCoordinatorOptions<Input, Output>['onSaved']
  private timer: ReturnType<typeof setTimeout> | undefined
  private latestInput: Input | undefined
  private queuedRevision = 0
  private savedRevision = 0
  private activeFlush: Promise<void> | undefined
  private disposed = false

  constructor(options: SaveCoordinatorOptions<Input, Output>) {
    this.delay = options.delay ?? 500
    this.saveInput = options.save
    this.onStateChange = options.onStateChange
    this.onSaved = options.onSaved
  }

  get revision() { return this.queuedRevision }
  get hasPendingChanges() { return this.savedRevision < this.queuedRevision }

  queue(input: Input): number {
    if (this.disposed) return this.queuedRevision
    this.latestInput = input
    this.queuedRevision += 1
    this.onStateChange?.('dirty')
    clearTimeout(this.timer)
    this.timer = setTimeout(() => void this.flush().catch(() => undefined), this.delay)
    return this.queuedRevision
  }

  async flush(): Promise<void> {
    clearTimeout(this.timer)
    if (this.activeFlush) {
      await this.activeFlush
      if (!this.hasPendingChanges) return
    }
    if (!this.latestInput || !this.hasPendingChanges) return

    this.activeFlush = this.drain()
    try {
      await this.activeFlush
    } finally {
      this.activeFlush = undefined
    }
  }

  private async drain(): Promise<void> {
    while (!this.disposed && this.latestInput && this.hasPendingChanges) {
      const input = this.latestInput
      const revision = this.queuedRevision
      this.onStateChange?.('saving')
      try {
        const output = await this.saveInput(input)
        this.savedRevision = Math.max(this.savedRevision, revision)
        const isLatest = revision === this.queuedRevision
        this.onSaved?.(output, revision, isLatest)
        this.onStateChange?.(isLatest ? 'saved' : 'dirty')
      } catch (error) {
        this.onStateChange?.('error', error)
        throw error
      }
    }
  }

  dispose() {
    this.disposed = true
    clearTimeout(this.timer)
  }
}
