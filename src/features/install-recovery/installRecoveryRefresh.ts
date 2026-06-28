export const INSTALL_RECOVERY_REFRESH_EVENT = "hmm:install-recovery-refresh";

export function notifyInstallRecoveryRefresh() {
  window.dispatchEvent(new Event(INSTALL_RECOVERY_REFRESH_EVENT));
}

export function subscribeInstallRecoveryRefresh(listener: () => void) {
  window.addEventListener(INSTALL_RECOVERY_REFRESH_EVENT, listener);
  return () => window.removeEventListener(INSTALL_RECOVERY_REFRESH_EVENT, listener);
}
