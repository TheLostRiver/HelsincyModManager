import type { AppRouteId } from "../routing/routeTypes";
import type { TourDefinition, TourStep } from "../../shared/onboarding/tourTypes";
import type { OnboardingTourCopy } from "./onboardingTourCopy";

export const ONBOARDING_ROUTE_ORDER: readonly AppRouteId[] = [
  "dashboard",
  "mods",
  "profiles",
  "settings",
];

export const OPTIONAL_ONBOARDING_ROUTE_IDS: readonly AppRouteId[] = [
  "recovery",
  "categories",
  "backups",
  "diagnostics",
  "about",
];

// 步骤语义（id/高亮目标/回退目标/位置/内边距）；标题、描述、要点与提示经
// onboardingTourCopy 按当次语言组装（语义/文本分离）。
type TourFeatureMeta = {
  id: string;
  target: string;
  fallbackTarget?: string;
  placement?: TourStep["placement"];
  spotlightPadding?: number;
};

const routeFeatureMeta: Record<AppRouteId, readonly TourFeatureMeta[]> = {
  dashboard: [
    { id: "dashboard-steam-scan", target: "dashboard.steam-scan", fallbackTarget: "dashboard.game-setup", placement: "bottom-start" },
    { id: "dashboard-manual-directory", target: "dashboard.manual-directory", fallbackTarget: "dashboard.game-setup", placement: "bottom-start" },
    { id: "dashboard-launch-game", target: "dashboard.launch-game", fallbackTarget: "dashboard.game-setup", placement: "top-start" },
    { id: "dashboard-prerequisites", target: "dashboard.prerequisites", fallbackTarget: "dashboard.game-setup", placement: "right-start" },
  ],
  mods: [
    { id: "mods-import", target: "mods.import-action", fallbackTarget: "mods.actions", placement: "bottom-start" },
    { id: "mods-library", target: "mods.library", placement: "top-start" },
    { id: "mods-lifecycle", target: "mods.actions", placement: "bottom-start" },
  ],
  recovery: [
    { id: "recovery-overview", target: "recovery.overview", fallbackTarget: "recovery.state", placement: "bottom-start" },
    { id: "recovery-actions", target: "recovery.manual-actions", fallbackTarget: "recovery.actions", placement: "top-start" },
    { id: "recovery-mods", target: "recovery.mods", fallbackTarget: "recovery.state-detail", placement: "top-start" },
  ],
  categories: [
    { id: "categories-create", target: "categories.create", placement: "bottom-start" },
    { id: "categories-manage", target: "categories.manage", placement: "top-start" },
  ],
  profiles: [
    { id: "profiles-list", target: "profiles.list", placement: "right-start" },
    { id: "profiles-directories", target: "profiles.save-directories", fallbackTarget: "profiles.settings", placement: "left-start" },
    { id: "profiles-manual-backup", target: "profiles.manual-backup", fallbackTarget: "profiles.settings", placement: "right-start" },
    { id: "profiles-auto-backup", target: "profiles.auto-backup", fallbackTarget: "profiles.settings", placement: "right-start" },
    { id: "profiles-backup-policy", target: "profiles.backup-policy", fallbackTarget: "profiles.settings", placement: "right-start" },
    { id: "profiles-history", target: "profiles.backup-history", fallbackTarget: "profiles.settings", placement: "top-start" },
  ],
  backups: [
    { id: "backups-filters", target: "backups.filters", placement: "bottom-start" },
    { id: "backups-maintenance", target: "backups.profiles", fallbackTarget: "page.backups", placement: "right-start" },
    { id: "backups-history", target: "backups.history", fallbackTarget: "page.backups", placement: "left-start" },
  ],
  diagnostics: [
    { id: "diagnostics-actions", target: "diagnostics.actions", placement: "bottom-start" },
    { id: "diagnostics-health", target: "diagnostics.health", fallbackTarget: "diagnostics.state", placement: "bottom-start" },
    { id: "diagnostics-logs", target: "diagnostics.logs", fallbackTarget: "page.diagnostics", placement: "top-start" },
  ],
  settings: [
    { id: "settings-background-protection", target: "settings.background-protection", fallbackTarget: "settings.save-backup", placement: "left-start" },
  ],
  about: [
    { id: "about-release", target: "about.release", placement: "bottom-start" },
    { id: "about-links", target: "about.links", placement: "top-start" },
  ],
};

type BuildOnboardingTourOptions = {
  includeWelcome?: boolean;
};

export function buildOnboardingTour(
  startRouteId: AppRouteId,
  copy: OnboardingTourCopy,
  { includeWelcome = false }: BuildOnboardingTourOptions = {},
): TourDefinition {
  const routeOrder = rotateRoutesFrom(startRouteId);
  const isPageLocalTour = !ONBOARDING_ROUTE_ORDER.includes(startRouteId);
  const steps: TourStep[] = includeWelcome ? [buildWelcomeStep(copy)] : [];

  routeOrder.forEach((routeId, index) => {
    const guidance = copy.routes[routeId];
    const featureMetaList = routeFeatureMeta[routeId];
    const isLastRoute = index === routeOrder.length - 1;

    if (isPageLocalTour) {
      steps.push({
        id: `page-${routeId}`,
        title: guidance.title,
        description: guidance.description,
        target: `page.${routeId}`,
        placement: "bottom-start",
        bullets: guidance.bullets,
        callout: copy.builder.pageLocalCallout,
        primaryLabel: copy.builder.pageLocalPrimary,
        spotlightPadding: 0,
        interaction: "blocked",
        advance: { kind: "controls" },
      });
    }

    featureMetaList.forEach((feature, featureIndex) => {
      const isLastFeature = featureIndex === featureMetaList.length - 1;
      const isFinalTourStep = isLastRoute && isLastFeature;
      const featureCopy = (guidance.features as Record<string, {
        title: string;
        description: string;
        bullets?: readonly string[];
        callout?: string;
      }>)[feature.id];

      steps.push({
        ...feature,
        title: featureCopy.title,
        description: featureCopy.description,
        ...(featureCopy.bullets ? { bullets: featureCopy.bullets } : {}),
        ...(featureCopy.callout ? { callout: featureCopy.callout } : {}),
        primaryLabel: isFinalTourStep ? copy.builder.finishLabel : copy.builder.continueLabel,
        spotlightPadding: feature.spotlightPadding ?? 6,
        interaction: "blocked",
        advance: isFinalTourStep ? { kind: "terminal" } : { kind: "controls" },
      });
    });

    if (!isLastRoute) {
      const nextRouteId = routeOrder[index + 1];
      const nextGuidance = copy.routes[nextRouteId];
      steps.push({
        id: `navigate-${nextRouteId}`,
        title: copy.builder.navigateTitle(nextGuidance.title),
        description: copy.builder.navigateDescription(nextGuidance.title),
        target: `nav.${nextRouteId}`,
        placement: "right-start",
        callout: copy.builder.navCallout,
        primaryLabel: copy.builder.waitClickLabel,
        spotlightPadding: 5,
        interaction: "target-only",
        advance: { kind: "route-change", expectedRouteId: nextRouteId },
      });
    }
  });

  return {
    id: isPageLocalTour ? `hmm.page-tour.${startRouteId}` : "hmm.first-run",
    contentVersion: isPageLocalTour ? 1 : 5,
    steps,
  };
}

export function rotateRoutesFrom(startRouteId: AppRouteId) {
  const startIndex = ONBOARDING_ROUTE_ORDER.indexOf(startRouteId);
  if (startIndex < 0) return [startRouteId];
  return [
    ...ONBOARDING_ROUTE_ORDER.slice(startIndex),
    ...ONBOARDING_ROUTE_ORDER.slice(0, startIndex),
  ];
}

function buildWelcomeStep(copy: OnboardingTourCopy): TourStep {
  return {
    id: "welcome",
    title: copy.welcome.title,
    description: copy.welcome.description,
    features: [
      { icon: "shield", ...copy.welcome.features.shield },
      { icon: "layers", ...copy.welcome.features.layers },
      { icon: "profiles", ...copy.welcome.features.profiles },
      { icon: "backup", ...copy.welcome.features.backup },
    ],
    callout: copy.welcome.callout,
    primaryLabel: copy.welcome.primaryLabel,
    interaction: "blocked",
    advance: { kind: "controls" },
  };
}
