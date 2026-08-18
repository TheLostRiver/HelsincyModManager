export type TourAnchorId = string;

export type TourPlacement =
  | "top"
  | "top-start"
  | "top-end"
  | "right"
  | "right-start"
  | "right-end"
  | "bottom"
  | "bottom-start"
  | "bottom-end"
  | "left"
  | "left-start"
  | "left-end";

export type TourFeatureIcon = "shield" | "layers" | "profiles" | "backup";

export type TourFeature = {
  icon: TourFeatureIcon;
  title: string;
  description: string;
};

export type TourInteractionPolicy = "blocked" | "target-only";

export type TourAdvancePolicy =
  | { kind: "controls" }
  | { kind: "route-change"; expectedRouteId: string }
  | { kind: "terminal" };

export type TourStep = {
  id: string;
  title: string;
  description: string;
  target?: TourAnchorId;
  fallbackTarget?: TourAnchorId;
  placement?: TourPlacement;
  features?: readonly TourFeature[];
  bullets?: readonly string[];
  callout?: string;
  primaryLabel: string;
  spotlightPadding?: number;
  interaction: TourInteractionPolicy;
  advance: TourAdvancePolicy;
};

export type TourDefinition = {
  id: string;
  contentVersion: number;
  steps: readonly TourStep[];
};

export type TourOutcome = "completed" | "skipped";
