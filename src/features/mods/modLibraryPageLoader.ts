import { refreshModLibraryDurableStatuses } from "./modLibraryRecoveryRefresh.ts";
import type { ModLibraryLoadContext } from "./useModLibraryQuery";
import type { ModLibraryPage, QueryModLibraryInput } from "./modLibraryTypes";
import type { InstallRecoverySummary, ScanInstallRecoveryInput } from "./modInstallPlanTypes";
import type { ModLibrarySessionStore } from "./modLibrarySessionStore";

export async function loadModLibraryPageWithStatuses(
  input: QueryModLibraryInput,
  context: ModLibraryLoadContext,
  services: {
    query: (input: QueryModLibraryInput) => Promise<ModLibraryPage>;
    scan: (input: ScanInstallRecoveryInput) => Promise<InstallRecoverySummary[]>;
    cache: ModLibrarySessionStore;
  },
): Promise<ModLibraryPage> {
  const page = await services.query(input);
  if (!context.isCurrent() || !input.profileContext) return page;
  const { gameId, profileId } = input.profileContext;
  if (gameId !== "mhw") throw { code: "game_id_invalid" };
  const result = await refreshModLibraryDurableStatuses(page.items, {
    profileId,
    isCurrent: context.isCurrent,
    loadRecoveryStatuses: (modIds) => services.scan({ gameId, profileId, modIds }),
  }, services.cache.pendingStatusModIds(gameId, profileId));
  if (context.isCurrent()) {
    services.cache.writeStatusSnapshot(gameId, profileId, context.generation, result.summaries, result.verified);
  }
  return { ...page, items: result.items, statusVerified: result.verified };
}
