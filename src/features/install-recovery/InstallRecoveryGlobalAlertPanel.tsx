import { AlertTriangle, CircleHelp } from "lucide-react";
import { useAppRoute } from "../../app/routing/useAppRoute";
import { useGameSetup } from "../game-setup/GameSetupProvider";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { deriveInstallRecoveryGlobalAlert, type InstallRecoveryGlobalAlertView } from "./installRecoveryGlobalAlert";
import { recoveryCenterCopy } from "./recoveryCenterCopy";
import { useInstallRecoveryHealth } from "./useInstallRecoveryHealth";

export function InstallRecoveryGlobalAlert() {
  const gameSetup = useGameSetup();
  const { locale } = useI18n();
  const alertCopy = resolveCopy(recoveryCenterCopy, locale).globalAlert;
  const recoveryHealth = useInstallRecoveryHealth({
    gameId: "mhw",
    enabled: gameSetup.status.kind === "configured",
  });
  const alert = deriveInstallRecoveryGlobalAlert(recoveryHealth, alertCopy);
  const { navigate } = useAppRoute();

  if (!alert) {
    return null;
  }

  return (
    <GlobalRecoveryAlertView
      alert={alert}
      panelAria={alertCopy.panelAria}
      onOpenRecoveryCenter={() => navigate("/recovery")}
    />
  );
}

function GlobalRecoveryAlertView({
  alert,
  panelAria,
  onOpenRecoveryCenter,
}: {
  alert: InstallRecoveryGlobalAlertView;
  panelAria: string;
  onOpenRecoveryCenter: () => void;
}) {
  return (
    <section
      className={`install-recovery-global-alert is-${alert.status}`}
      aria-label={panelAria}
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
