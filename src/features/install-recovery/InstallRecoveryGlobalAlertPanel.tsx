import { AlertTriangle, CircleHelp } from "lucide-react";
import { useAppRoute } from "../../app/routing/useAppRoute";
import { useGameSetup } from "../game-setup/GameSetupProvider";
import { deriveInstallRecoveryGlobalAlert, type InstallRecoveryGlobalAlertView } from "./installRecoveryGlobalAlert";
import { useInstallRecoveryHealth } from "./useInstallRecoveryHealth";

export function InstallRecoveryGlobalAlert() {
  const gameSetup = useGameSetup();
  const recoveryHealth = useInstallRecoveryHealth({
    gameId: "mhw",
    enabled: gameSetup.status.kind === "configured",
  });
  const alert = deriveInstallRecoveryGlobalAlert(recoveryHealth);
  const { navigate } = useAppRoute();

  if (!alert) {
    return null;
  }

  return <GlobalRecoveryAlertView alert={alert} onOpenRecoveryCenter={() => navigate("/recovery")} />;
}

function GlobalRecoveryAlertView({
  alert,
  onOpenRecoveryCenter,
}: {
  alert: InstallRecoveryGlobalAlertView;
  onOpenRecoveryCenter: () => void;
}) {
  return (
    <section
      className={`install-recovery-global-alert is-${alert.status}`}
      aria-label="安装恢复全局告警"
      role={alert.status === "attention" ? "alert" : "status"}
      aria-live={alert.status === "attention" ? "assertive" : "polite"}
    >
      <div className="install-recovery-global-alert__icon" aria-hidden="true">
        {alert.status === "attention" ? <AlertTriangle size={18} /> : <CircleHelp size={18} />}
      </div>
      <div className="install-recovery-global-alert__copy">
        <strong>{alert.title}</strong>
        <p>{alert.description}</p>
      </div>
      <button type="button" onClick={onOpenRecoveryCenter}>
        {alert.actionLabel}
      </button>
    </section>
  );
}
