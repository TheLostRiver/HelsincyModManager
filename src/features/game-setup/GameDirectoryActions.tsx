import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Search } from "lucide-react";

type GameDirectoryActionsProps = {
  isBusy: boolean;
  onDirectorySelected: (directory: string) => Promise<void>;
  onScanSteam: () => Promise<void>;
};

export function GameDirectoryActions({ isBusy, onDirectorySelected, onScanSteam }: GameDirectoryActionsProps) {
  async function handleManualSelect() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择《怪物猎人：世界 冰原》游戏目录",
    });

    if (typeof selected === "string") {
      await onDirectorySelected(selected);
    }
  }

  return (
    <div className="setup-actions">
      <button type="button" className="primary-action" disabled={isBusy} onClick={() => void onScanSteam()}>
        <Search size={16} />
        自动扫描 Steam
      </button>
      <button type="button" className="secondary-action" disabled={isBusy} onClick={() => void handleManualSelect()}>
        <FolderOpen size={16} />
        手动选择游戏目录
      </button>
    </div>
  );
}
