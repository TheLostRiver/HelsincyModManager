import { MAX_MOD_INSTALLATION_STATE_IDS } from "./modInstallationStateTypes.ts";
import type { ModInstallationStateRequest, ModInstallationStateUpdate } from "./modInstallationStateTypes";
import type { ModLibraryLoadContext } from "./useModLibraryQuery";
import type { ModLibraryPage, QueryModLibraryInput } from "./modLibraryTypes";
import type { InstallRecoverySummary, ScanInstallRecoveryInput } from "./modInstallPlanTypes";
import type { ModLibrarySessionStore } from "./modLibrarySessionStore";

export async function loadModLibraryPageWithStatuses(
  input: QueryModLibraryInput,
  context: ModLibraryLoadContext,
  services: {
    query: (input: QueryModLibraryInput) => Promise<ModLibraryPage>;
    states: (input: ModInstallationStateRequest) => Promise<ModInstallationStateUpdate>;
    scan: (input: ScanInstallRecoveryInput) => Promise<InstallRecoverySummary[]>;
    cache: ModLibrarySessionStore;
  },
): Promise<ModLibraryPage> {
  const page = (!context.refresh && services.cache.readCatalogPage(input)) || await services.query(input);
  if (!context.isCurrent() || !input.profileContext) return page;
  const { gameId, profileId } = input.profileContext;
  if (gameId !== "mhw") throw { code: "game_id_invalid" };
  // Reconcile every retained card, including off-page task targets. This bounded metadata
  // read also repairs missing events for Mods affected by shared ownership changes.
  const modIds = [...new Set([...page.items.map((item) => item.id),
    ...services.cache.cachedModIds(gameId, profileId), ...services.cache.pendingStatusModIds(gameId, profileId)])];
  let verified = true;
  try {
    for (let offset = 0; offset < modIds.length; offset += MAX_MOD_INSTALLATION_STATE_IDS) {
      if (!context.isCurrent()) return page;
      const requested = modIds.slice(offset, offset + MAX_MOD_INSTALLATION_STATE_IDS);
      const update = await services.states({ gameId, profileId, modIds: requested });
      if (!context.isCurrent()) return page;
      if (update.gameId !== gameId || update.profileId !== profileId || requested.some((id) => !update.modIds.includes(id))
        || !services.cache.acceptInstallationStates(update, context.generation)) throw new Error("incomplete installation state");
    }
    // Explicit refresh and the recovery center retain file integrity verification.
    // The API wrapper records it independently and rejects scans crossing a write.
    if (context.refresh && modIds.length > 0 && context.isCurrent()) {
      await services.scan({ gameId, profileId, modIds });
    }
  } catch {
    verified = false;
  }
  if (!context.isCurrent()) return page;
  services.cache.finishStatusRead(gameId, profileId, context.generation, modIds, verified);
  return { ...services.cache.projectPage(page, input.profileContext), statusVerified: verified };
}
