import {
  applyInstallManifestUnavailable,
  applyInstallRecoverySummaries,
} from "./modLibraryLoadState.ts";
import type { InstallRecoverySummary } from "./modInstallPlanTypes";
import type { ModLibraryItem } from "./modLibraryTypes";

type ModLibraryDurableStatusLoaders = {
  profileId: string;
  loadRecoveryStatuses: (modIds: string[]) => Promise<InstallRecoverySummary[]>;
  isCurrent?: () => boolean;
};

export type ModLibraryDurableStatusRefresh = {
  items: ModLibraryItem[];
  verified: boolean;
  summaries: InstallRecoverySummary[];
};

export async function refreshModLibraryDurableStatuses(
  items: ModLibraryItem[],
  loaders: ModLibraryDurableStatusLoaders,
  additionalModIds: string[] = [],
): Promise<ModLibraryDurableStatusRefresh> {
  const modIds = Array.from(new Set([...items.map((item) => item.id), ...additionalModIds])).filter((id) => id.length > 0);
  if (modIds.length === 0) {
    return { items, verified: true, summaries: [] };
  }
  try {
    if (loaders.isCurrent?.() === false) throw new Error("superseded");
    const recoveryStatuses = await loaders.loadRecoveryStatuses(modIds);
    if (loaders.isCurrent?.() === false) throw new Error("superseded");
    const byId = new Map(recoveryStatuses.map((summary) => [summary.modId, summary]));
    if (byId.size !== recoveryStatuses.length
      || recoveryStatuses.some((summary) => summary.profileId !== loaders.profileId || !modIds.includes(summary.modId))
      || modIds.some((id) => {
      const summary = byId.get(id);
      return !summary || summary.profileId !== loaders.profileId;
    })) throw new Error("incomplete recovery facts");
    return {
      items: applyInstallRecoverySummaries(items, recoveryStatuses),
      verified: true,
      summaries: recoveryStatuses,
    };
  } catch {
    return {
      items: applyInstallManifestUnavailable(items),
      verified: false,
      summaries: [],
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
