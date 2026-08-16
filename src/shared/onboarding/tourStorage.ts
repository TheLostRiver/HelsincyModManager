import type { TourDefinition, TourOutcome } from "./tourTypes";

export const ONBOARDING_STORAGE_KEY = "helsincy.onboarding";

type StoredTourProgress = {
  contentVersion: number;
  outcome: TourOutcome;
};

type StoredOnboardingState = {
  schemaVersion: 1;
  tours: Record<string, StoredTourProgress>;
};

export type TourStorage = Pick<Storage, "getItem" | "setItem">;

const EMPTY_STATE: StoredOnboardingState = {
  schemaVersion: 1,
  tours: {},
};

export function shouldAutoStartTour(definition: TourDefinition, storage: TourStorage | null) {
  const progress = readOnboardingState(storage).tours[definition.id];
  return !progress || progress.contentVersion < definition.contentVersion;
}

export function saveTourOutcome(
  definition: TourDefinition,
  outcome: TourOutcome,
  storage: TourStorage | null,
) {
  if (!storage) return false;

  const state = readOnboardingState(storage);
  const nextState: StoredOnboardingState = {
    schemaVersion: 1,
    tours: {
      ...state.tours,
      [definition.id]: {
        contentVersion: definition.contentVersion,
        outcome,
      },
    },
  };

  try {
    storage.setItem(ONBOARDING_STORAGE_KEY, JSON.stringify(nextState));
    return true;
  } catch {
    return false;
  }
}

export function readOnboardingState(storage: TourStorage | null): StoredOnboardingState {
  if (!storage) return EMPTY_STATE;

  try {
    const raw = storage.getItem(ONBOARDING_STORAGE_KEY);
    if (!raw) return EMPTY_STATE;

    const parsed: unknown = JSON.parse(raw);
    if (!isStoredOnboardingState(parsed)) return EMPTY_STATE;
    return parsed;
  } catch {
    return EMPTY_STATE;
  }
}

function isStoredOnboardingState(value: unknown): value is StoredOnboardingState {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<StoredOnboardingState>;
  if (candidate.schemaVersion !== 1 || !candidate.tours || typeof candidate.tours !== "object") {
    return false;
  }

  return Object.values(candidate.tours).every((progress) => {
    if (!progress || typeof progress !== "object") return false;
    const item = progress as Partial<StoredTourProgress>;
    return Number.isInteger(item.contentVersion)
      && (item.contentVersion ?? 0) > 0
      && (item.outcome === "completed" || item.outcome === "skipped");
  });
}
