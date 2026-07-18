import {
  applyInstallManifestUnavailable,
  applyInstallManifestStatusSummaries,
  applyInstallRecoverySummaries,
  applyInstallRecoveryUnavailable,
} from "./modLibraryLoadState";
import type {
  InstallManifestStatusSummary,
  InstallRecoverySummary,
} from "./modInstallPlanTypes";
import type { ModLibraryItem } from "./modLibraryTypes";

type ModLibraryDurableStatusLoaders = {
  loadManifestStatuses: (modIds: string[]) => Promise<InstallManifestStatusSummary[]>;
  loadRecoveryStatuses: (modIds: string[]) => Promise<InstallRecoverySummary[]>;
};

export type ModLibraryDurableStatusRefresh = {
  items: ModLibraryItem[];
  verified: boolean;
};

export async function refreshModLibraryDurableStatuses(
  items: ModLibraryItem[],
  loaders: ModLibraryDurableStatusLoaders,
): Promise<ModLibraryDurableStatusRefresh> {
  const modIds = Array.from(new Set(items.map((item) => item.id))).filter((id) => id.length > 0);
  if (modIds.length === 0) {
    return { items, verified: true };
  }

  let itemsWithManifestStatus: ModLibraryItem[];
  try {
    const manifestStatuses = await loaders.loadManifestStatuses(modIds);
    itemsWithManifestStatus = applyInstallManifestStatusSummaries(items, manifestStatuses);
  } catch {
    return {
      items: applyInstallManifestUnavailable(items),
      verified: false,
    };
  }

  try {
    const recoveryStatuses = await loaders.loadRecoveryStatuses(modIds);
    return {
      items: applyInstallRecoverySummaries(itemsWithManifestStatus, recoveryStatuses),
      verified: true,
    };
  } catch {
    return {
      items: applyInstallRecoveryUnavailable(itemsWithManifestStatus),
      verified: false,
    };
  }
}

export function createModLibraryStatusProbe(modId: string, modName: string): ModLibraryItem {
  return {
    id: modId,
    name: modName,
    sizeLabel: "",
    status: "unknown",
    categoryLabels: [],
  };
}
