import { isUnsafeInstallStatus } from "./modLibraryLoadState.ts";
import type { InstallManifestStatusSummary, InstallRecoverySummary } from "./modInstallPlanTypes";
import type { ModInstallationStateUpdate } from "./modInstallationStateTypes";
import type { ModInstallSummary, ModLibraryItem, ModLibraryPage } from "./modLibraryTypes";

type ScopeState = {
  epoch: string | null;
  retiredEpochs: Set<string>;
  resetRevision: number;
  states: Map<string, { revision: number; summary: InstallManifestStatusSummary }>;
  integrity: Map<string, { sequence: number; summary: InstallRecoverySummary }>;
  integrityReset: number;
  integrityFailure: number;
  unavailable: boolean;
  availabilityRevision: number;
};

const keyFor = (gameId: string, profileId: string) => JSON.stringify([gameId, profileId]);

function sameSummary(left: ModInstallSummary | undefined, right: ModInstallSummary) {
  return left?.status === right.status && left.managedFileCount === right.managedFileCount
    && left.backupCount === right.backupCount && left.adoptedFileCount === right.adoptedFileCount
    && left.recoveryStatus === right.recoveryStatus && left.issueCount === right.issueCount
    && left.issues === right.issues;
}

export function createModInstallationStateCache() {
  const scopes = new Map<string, ScopeState>();
  const projections = new WeakMap<ModLibraryPage, { key: string; version: number; page: ModLibraryPage }>();
  let version = 0;
  const scopeFor = (gameId: string, profileId: string) => {
    const key = keyFor(gameId, profileId);
    let scope = scopes.get(key);
    if (!scope) {
      if (scopes.size >= 16) scopes.delete(scopes.keys().next().value!);
      scope = { epoch: null, retiredEpochs: new Set(), resetRevision: 0, states: new Map(),
        integrity: new Map(), integrityReset: 0, integrityFailure: 0, unavailable: false, availabilityRevision: 0 };
      scopes.set(key, scope);
    }
    return scope;
  };

  const effectiveSummary = (scope: ScopeState, profileId: string, modId: string): InstallManifestStatusSummary | undefined => {
    const known = scope.states.get(modId)?.summary;
    const integrity = scope.integrity.get(modId);
    const failedIntegrity = scope.integrityFailure > (integrity?.sequence ?? 0);
    const issue = integrity?.summary;
    const unsafe = issue && isUnsafeInstallStatus(issue.status) ? issue : undefined;
    if (!known && !unsafe && !failedIntegrity && !scope.unavailable && scope.epoch === null) return undefined;
    const unsafeStatus = issue && isUnsafeInstallStatus(issue.status) ? issue.status : undefined;
    return {
      profileId, modId,
      status: scope.unavailable || failedIntegrity ? "unknown" : unsafeStatus ?? known?.status ?? "unknown",
      managedFileCount: known?.managedFileCount ?? unsafe?.managedFileCount ?? 0,
      backupCount: known?.backupCount ?? unsafe?.backupCount ?? 0,
      adoptedFileCount: known?.adoptedFileCount ?? unsafe?.adoptedFileCount,
      installedRevisionId: known?.installedRevisionId ?? null,
    };
  };

  return {
    getVersion: () => version,
    apply: (update: ModInstallationStateUpdate, source: "event" | "query") => {
      const scope = scopeFor(update.gameId, update.profileId);
      if (!update.epoch || !Number.isSafeInteger(update.revision) || update.revision < 1
        || new Set(update.modIds).size !== update.modIds.length
        || (update.available && (update.summaries.length !== update.modIds.length
          || new Set(update.summaries.map((summary) => summary.modId)).size !== update.modIds.length
          || update.summaries.some((summary) => summary.profileId !== update.profileId || !update.modIds.includes(summary.modId))))) {
        scope.unavailable = true;
        version++;
        return false;
      }
      if (scope.retiredEpochs.has(update.epoch)) return false;
      if (scope.epoch !== update.epoch) {
        // A different event epoch cannot displace a confirmed query. The next narrow
        // query establishes the current backend session; old event transports may lag.
        if (scope.epoch !== null && source === "event") return false;
        if (scope.epoch !== null) scope.retiredEpochs.add(scope.epoch);
        scope.epoch = update.epoch;
        scope.resetRevision = 0;
        scope.availabilityRevision = 0;
        scope.states.clear();
      }
      if (update.revision < scope.resetRevision) return false;
      if (update.reset && update.revision > scope.resetRevision) {
        for (const [id, state] of scope.states) if (state.revision <= update.revision) scope.states.delete(id);
        scope.resetRevision = update.revision;
      }
      for (const summary of update.summaries) {
        if ((scope.states.get(summary.modId)?.revision ?? 0) >= update.revision) continue;
        scope.states.set(summary.modId, { revision: update.revision, summary });
      }
      // Do not use a global high-water mark to discard another Mod's older event.
      // Per-Mod revisions and reset barriers handle both reorderings independently.
      if ((source === "query" || !update.available) && update.revision >= scope.availabilityRevision) {
        scope.unavailable = !update.available;
        scope.availabilityRevision = update.revision;
      }
      version++;
      return update.available;
    },
    unavailable: (gameId: string, profileId: string) => {
      scopeFor(gameId, profileId).unavailable = true;
      version++;
    },
    summaries: (gameId: string, profileId: string) => {
      const scope = scopeFor(gameId, profileId);
      return [...scope.states.keys()].map((id) => effectiveSummary(scope, profileId, id)!);
    },
    recordIntegrity: (gameId: string, profileId: string, ids: string[], sequence: number, summaries: InstallRecoverySummary[] | null) => {
      const scope = scopeFor(gameId, profileId);
      if (sequence < scope.integrityReset) return;
      if (summaries === null) {
        if (ids.length === 0) scope.integrityFailure = Math.max(scope.integrityFailure, sequence);
        else for (const id of ids) {
          if ((scope.integrity.get(id)?.sequence ?? 0) > sequence) continue;
          const known = scope.states.get(id)?.summary;
          scope.integrity.set(id, { sequence, summary: { profileId, modId: id, status: "unknown",
            managedFileCount: known?.managedFileCount ?? 0, backupCount: known?.backupCount ?? 0,
            adoptedFileCount: known?.adoptedFileCount ?? 0, issueCount: 0, issues: [] } });
        }
      } else {
        if (ids.length === 0) {
          scope.integrityReset = sequence;
          for (const [id, fact] of scope.integrity) if (fact.sequence <= sequence) scope.integrity.delete(id);
          if (scope.integrityFailure <= sequence) scope.integrityFailure = 0;
        }
        for (const summary of summaries) {
          if ((scope.integrity.get(summary.modId)?.sequence ?? 0) > sequence) continue;
          scope.integrity.set(summary.modId, { sequence, summary });
        }
      }
      version++;
    },
    project: (page: ModLibraryPage, gameId: string, profileId: string): ModLibraryPage => {
      const key = keyFor(gameId, profileId);
      const previous = projections.get(page);
      if (previous?.key === key && previous.version === version) return previous.page;
      const scope = scopeFor(gameId, profileId);
      const sourceItems = previous?.key === key ? previous.page.items : page.items;
      let changed = false;
      const items = sourceItems.map((item): ModLibraryItem => {
        const state = effectiveSummary(scope, profileId, item.id);
        if (!state) return item;
        const integrity = scope.integrity.get(item.id)?.summary;
        const unsafe = integrity && isUnsafeInstallStatus(integrity.status) ? integrity : undefined;
        const summary: ModInstallSummary = {
          status: state.status, managedFileCount: state.managedFileCount, backupCount: state.backupCount,
          ...(state.adoptedFileCount === undefined ? {} : { adoptedFileCount: state.adoptedFileCount }),
          ...(unsafe ? { recoveryStatus: unsafe.status, issueCount: unsafe.issueCount, issues: unsafe.issues } : {}),
        };
        const status = isUnsafeInstallStatus(state.status) ? state.status
          : item.status === "disabled" || item.status === "conflict" ? item.status : state.status;
        if (item.status === status && sameSummary(item.installSummary, summary)) return item;
        changed = true;
        return { ...item, status, installSummary: summary };
      });
      const projected = changed ? { ...page, items } : previous?.key === key ? previous.page : page;
      projections.set(page, { key, version, page: projected });
      return projected;
    },
  };
}
