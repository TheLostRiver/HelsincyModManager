import { Target } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ModalSurface } from "../../shared/feedback/ModalSurface";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { GameId } from "../game-setup/gameSetupTypes";
import type { InstallManifestStatus } from "../mods/modInstallPlanTypes";
import { getModDetail } from "../mods/modLibraryApi";
import { EquipmentRetargetPanel } from "./EquipmentRetargetPanel";
import { retargetDialogCopy } from "./retargetDialogCopy";
import "./EquipmentRetargetDialog.css";

type EquipmentRetargetDialogProps = {
  gameId: GameId;
  modId: string;
  modName: string;
  profileId: string | null;
  installStatus: InstallManifestStatus | undefined;
  onClose: () => void;
  onSaved: () => Promise<void> | void;
};

export function EquipmentRetargetDialog({ gameId, modId, modName, profileId, installStatus, onClose, onSaved }: EquipmentRetargetDialogProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(retargetDialogCopy, locale);
  const [displayName, setDisplayName] = useState(modName);
  const [busy, setBusy] = useState(false);
  const [completedLocally, setCompletedLocally] = useState(false);
  const [currentInstallStatus, setCurrentInstallStatus] = useState(installStatus);

  useEffect(() => {
    let disposed = false;
    void getModDetail({ modId }).then((detail) => {
      if (!disposed && detail?.id === modId) setDisplayName(detail.name);
    }).catch(() => {});
    return () => { disposed = true; };
  }, [modId]);

  useEffect(() => { setCurrentInstallStatus(installStatus); }, [installStatus]);

  const onInstallCompleted = useCallback(async () => {
    setCompletedLocally(true);
    await onSaved();
    setCurrentInstallStatus("installed");
    setCompletedLocally(false);
  }, [onSaved]);

  return <ModalSurface kind="dialog" open title={copy.title} description={displayName} icon={<Target size={20} />}
    panelClassName="equipment-retarget-dialog" busy={busy} onClose={onClose}>
    <EquipmentRetargetPanel gameId={gameId} modId={modId} profileId={profileId}
      installStatus={currentInstallStatus} completedLocally={completedLocally}
      onBusyChange={setBusy} onInstallCompleted={onInstallCompleted} />
  </ModalSurface>;
}
