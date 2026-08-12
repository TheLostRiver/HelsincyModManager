export type BackgroundProtectionAutoVerificationDecision = "continue" | "complete";

type TimerHandle = ReturnType<typeof globalThis.setTimeout>;

type BackgroundProtectionAutoVerificationOptions = {
  verify: () => Promise<BackgroundProtectionAutoVerificationDecision>;
  isBusy: () => boolean;
  onActiveChange: (active: boolean) => void;
  delaysMs?: readonly number[];
  busyRetryDelayMs?: number;
  setTimer?: (callback: () => void, delayMs: number) => TimerHandle;
  clearTimer?: (handle: TimerHandle) => void;
};

// Verify almost immediately, once more at roughly 3 seconds, then at 1, 5, 10,
// and 16 minutes after enable. The short convergence read covers a worker
// heartbeat that lands while the first Windows task inspection is still running.
export const BACKGROUND_PROTECTION_AUTO_VERIFICATION_DELAYS_MS = [
  750,
  2_250,
  57_000,
  4 * 60_000,
  5 * 60_000,
  6 * 60_000,
] as const;

const DEFAULT_BUSY_RETRY_DELAY_MS = 2_000;

export class BackgroundProtectionAutoVerificationScheduler {
  private readonly verify: BackgroundProtectionAutoVerificationOptions["verify"];
  private readonly isBusy: BackgroundProtectionAutoVerificationOptions["isBusy"];
  private readonly onActiveChange: BackgroundProtectionAutoVerificationOptions["onActiveChange"];
  private readonly delaysMs: readonly number[];
  private readonly busyRetryDelayMs: number;
  private readonly setTimer: NonNullable<BackgroundProtectionAutoVerificationOptions["setTimer"]>;
  private readonly clearTimer: NonNullable<BackgroundProtectionAutoVerificationOptions["clearTimer"]>;
  private timer: TimerHandle | null = null;
  private nextDelayIndex = 0;
  private active = false;
  private disposed = false;

  constructor(options: BackgroundProtectionAutoVerificationOptions) {
    this.verify = options.verify;
    this.isBusy = options.isBusy;
    this.onActiveChange = options.onActiveChange;
    this.delaysMs = options.delaysMs ?? BACKGROUND_PROTECTION_AUTO_VERIFICATION_DELAYS_MS;
    this.busyRetryDelayMs = options.busyRetryDelayMs ?? DEFAULT_BUSY_RETRY_DELAY_MS;
    this.setTimer = options.setTimer ?? globalThis.setTimeout;
    this.clearTimer = options.clearTimer ?? globalThis.clearTimeout;
  }

  arm() {
    if (this.disposed) return;
    this.clearPendingTimer();
    this.nextDelayIndex = 0;
    this.setActive(true);
    this.scheduleNextVerification();
  }

  cancel() {
    this.clearPendingTimer();
    this.nextDelayIndex = 0;
    this.setActive(false);
  }

  dispose() {
    this.cancel();
    this.disposed = true;
  }

  isActive() {
    return this.active;
  }

  private setActive(active: boolean) {
    if (this.active === active) return;
    this.active = active;
    this.onActiveChange(active);
  }

  private clearPendingTimer() {
    if (this.timer === null) return;
    this.clearTimer(this.timer);
    this.timer = null;
  }

  private scheduleNextVerification() {
    if (!this.active || this.disposed || this.timer !== null) return;
    const delayMs = this.delaysMs[this.nextDelayIndex];
    if (delayMs === undefined) {
      this.cancel();
      return;
    }

    this.timer = this.setTimer(() => {
      this.timer = null;
      void this.runVerification();
    }, delayMs);
  }

  private scheduleBusyRetry() {
    if (!this.active || this.disposed || this.timer !== null) return;
    this.timer = this.setTimer(() => {
      this.timer = null;
      void this.runVerification();
    }, this.busyRetryDelayMs);
  }

  private async runVerification() {
    if (!this.active || this.disposed) return;
    if (this.isBusy()) {
      this.scheduleBusyRetry();
      return;
    }

    this.nextDelayIndex += 1;
    let decision: BackgroundProtectionAutoVerificationDecision = "continue";
    try {
      decision = await this.verify();
    } catch {
      // A temporary read failure must not cancel the remaining verification points.
    }

    if (!this.active || this.disposed) return;
    if (decision === "complete") {
      this.cancel();
      return;
    }
    this.scheduleNextVerification();
  }
}
