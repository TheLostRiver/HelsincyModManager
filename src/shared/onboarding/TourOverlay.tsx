import {
  ArrowLeft,
  ArrowRight,
  ArchiveRestore,
  Check,
  CheckCircle2,
  Layers3,
  Lightbulb,
  LoaderCircle,
  MapPinned,
  MousePointerClick,
  RefreshCw,
  Rocket,
  ShieldCheck,
  Sparkles,
  UserRoundCog,
  X,
} from "lucide-react";
import {
  autoUpdate,
  flip,
  offset,
  shift,
  size,
  useFloating,
  type Placement,
} from "@floating-ui/react";
import {
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type RefObject,
} from "react";
import { FeedbackPortal } from "../feedback/FeedbackProvider";
import { resolveCopy, useI18n } from "../i18n";
import { tourOverlayCopy } from "./tourOverlayCopy";
import { getFocusableElements } from "../feedback/focusTrap";
import { useModalFocusTrap } from "../feedback/useModalFocusTrap";
import { shouldDockTourPanel } from "./tourGeometry";
import type { TourFeatureIcon, TourOutcome, TourStep } from "./tourTypes";
import { useTourTarget } from "./useTourTarget";

type TourOverlayProps = {
  steps: readonly TourStep[];
  stepIndex: number;
  onStepChange: (index: number) => void;
  onTargetActivate?: (stepId: string) => void;
  onFinish: (outcome: TourOutcome) => void;
};

const EMPTY_TOUR_STEP: TourStep = {
  id: "tour-step-unavailable",
  title: "",
  description: "",
  primaryLabel: "",
  interaction: "blocked",
  advance: { kind: "terminal" },
};

type TourPhase = "opening" | "open" | "closing";

const TOUR_TRANSITION_MS = 320;
const TOUR_PANEL_RELOCATION_MS = 460;

type StableTourPanelLayout =
  | { kind: "welcome"; stepId: string }
  | { kind: "docked"; stepId: string }
  | { kind: "floating"; stepId: string; x: number; y: number; style: CSSProperties };

export function TourOverlay({
  steps,
  stepIndex: requestedStepIndex,
  onStepChange,
  onTargetActivate,
  onFinish,
}: TourOverlayProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(tourOverlayCopy, locale);
  const stepIndex = steps.length === 0
    ? 0
    : Math.min(Math.max(requestedStepIndex, 0), steps.length - 1);
  const step = steps[stepIndex] ?? EMPTY_TOUR_STEP;
  const positionerRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLElement | null>(null);
  const primaryActionRef = useRef<HTMLButtonElement | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  const openingTimerRef = useRef<number | null>(null);
  const previousStepIndexRef = useRef(stepIndex);
  const [phase, setPhase] = useState<TourPhase>("opening");
  const targetState = useTourTarget(
    step.target,
    step.fallbackTarget,
    step.spotlightPadding ?? 6,
  );
  const viewportSize = useViewportSize();
  const maskId = `tour-mask-${useId().replaceAll(":", "")}`;
  const hasRequestedTarget = Boolean(step.target || step.fallbackTarget);
  const isTargetUnavailable = hasRequestedTarget && !targetState.rect && targetState.timedOut;
  const isTargetPending = hasRequestedTarget && !targetState.rect && !isTargetUnavailable;
  const isAwaitingRouteChange = step.advance.kind === "route-change";
  const canGoPrevious = stepIndex > 0 && steps[stepIndex - 1]?.advance.kind !== "route-change";
  const stepDirection = stepIndex < previousStepIndexRef.current ? "backward" : "forward";

  const floating = useFloating({
    open: Boolean(targetState.element),
    strategy: "fixed",
    placement: (step.placement ?? "right-start") as Placement,
    whileElementsMounted: autoUpdate,
    middleware: [
      offset(16),
      flip({ padding: 16, fallbackAxisSideDirection: "end" }),
      shift({ padding: 16 }),
      size({
        padding: 16,
        apply({ availableHeight, availableWidth, elements }) {
          elements.floating.style.setProperty(
            "--tour-available-width",
            `${Math.max(0, availableWidth)}px`,
          );
          elements.floating.style.setProperty(
            "--tour-available-height",
            `${Math.max(0, availableHeight)}px`,
          );
        },
      }),
    ],
  });
  const panelLayout = useStableTourPanelLayout({
    stepId: step.id,
    hasRequestedTarget,
    targetRect: targetState.rect,
    shouldDock: shouldDockTourPanel(
      targetState.rect,
      viewportSize.width,
      viewportSize.height,
    ),
    floatingX: floating.x,
    floatingY: floating.y,
    floatingStyle: floating.floatingStyles,
    floatingPositioned: floating.isPositioned,
  });
  const isDocked = panelLayout.kind === "docked";

  const setPositionerRef = useCallback((node: HTMLDivElement | null) => {
    positionerRef.current = node;
    floating.refs.setFloating(node);
  }, [floating.refs]);

  useTourPanelRelocation(positionerRef, panelRef, phase === "open");

  useEffect(() => {
    floating.refs.setReference(targetState.element);
  }, [floating.refs, targetState.element]);

  useEffect(() => {
    previousStepIndexRef.current = stepIndex;
  }, [stepIndex]);

  useEffect(() => {
    openingTimerRef.current = window.setTimeout(() => {
      openingTimerRef.current = null;
      setPhase("open");
    }, getTourTransitionMillis());

    return () => {
      if (openingTimerRef.current !== null) window.clearTimeout(openingTimerRef.current);
      if (closeTimerRef.current !== null) window.clearTimeout(closeTimerRef.current);
    };
  }, []);

  const requestFinish = useCallback((outcome: TourOutcome) => {
    if (phase === "closing") return;
    setPhase("closing");
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null;
      onFinish(outcome);
    }, getTourTransitionMillis());
  }, [onFinish, phase]);

  const goPrevious = useCallback(() => {
    if (canGoPrevious) onStepChange(stepIndex - 1);
  }, [canGoPrevious, onStepChange, stepIndex]);

  const goNext = useCallback(() => {
    if (isTargetPending || step.advance.kind === "route-change") return;
    if (step.advance.kind === "terminal") {
      requestFinish("completed");
      return;
    }
    onStepChange(stepIndex + 1);
  }, [isTargetPending, onStepChange, requestFinish, step.advance.kind, stepIndex]);

  const notifyTargetActivated = useCallback(() => {
    if (!isAwaitingRouteChange || phase === "closing") return;
    onTargetActivate?.(step.id);
  }, [isAwaitingRouteChange, onTargetActivate, phase, step.id]);

  useModalFocusTrap({
    active: phase !== "closing" && step.interaction === "blocked",
    containerRef: panelRef,
    closeOnEscape: phase !== "closing",
    onRequestClose: () => requestFinish("skipped"),
    focusKey: step.id,
    initialFocusRef: primaryActionRef,
  });

  useEffect(() => {
    if (phase === "closing" || isTargetPending || isAwaitingRouteChange) return undefined;
    const frameId = window.requestAnimationFrame(() => primaryActionRef.current?.focus());
    return () => window.cancelAnimationFrame(frameId);
  }, [isAwaitingRouteChange, isTargetPending, phase, step.id]);

  useEffect(() => {
    if (phase === "closing" || !isAwaitingRouteChange) return undefined;
    const target = targetState.element;
    const frameId = target
      ? window.requestAnimationFrame(() => target.focus())
      : null;
    if (target) {
      // Capture the activation before the navigation button's own click handler changes
      // the route. This lets TourProvider require both activation and the expected route.
      target.addEventListener("click", notifyTargetActivated, true);
    }
    const handleTargetOnlyKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        requestFinish("skipped");
        return;
      }

      if (event.key !== "Tab") return;
      const panel = panelRef.current;
      const target = targetState.element;
      if (!panel || !target) return;

      const targetFocusables = target.tabIndex >= 0 ? [target] : getFocusableElements(target);
      const allowedFocusTargets = [...targetFocusables, ...getFocusableElements(panel)];
      if (allowedFocusTargets.length === 0) return;

      const activeElement = document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
      const currentIndex = activeElement ? allowedFocusTargets.indexOf(activeElement) : -1;
      const direction = event.shiftKey ? -1 : 1;
      const nextIndex = currentIndex < 0
        ? (event.shiftKey ? allowedFocusTargets.length - 1 : 0)
        : (currentIndex + direction + allowedFocusTargets.length) % allowedFocusTargets.length;

      event.preventDefault();
      allowedFocusTargets[nextIndex]?.focus();
    };
    document.addEventListener("keydown", handleTargetOnlyKeyDown, true);
    return () => {
      if (frameId !== null) window.cancelAnimationFrame(frameId);
      if (target) target.removeEventListener("click", notifyTargetActivated, true);
      document.removeEventListener("keydown", handleTargetOnlyKeyDown, true);
    };
  }, [isAwaitingRouteChange, notifyTargetActivated, phase, requestFinish, targetState.element]);

  const handlePanelKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.key === "Enter" && event.target === primaryActionRef.current) {
      event.preventDefault();
      goNext();
      return;
    }
    if (event.key === "ArrowLeft" && canGoPrevious) {
      event.preventDefault();
      goPrevious();
    }
    if (event.key === "ArrowRight" && !isTargetPending && step.advance.kind === "controls") {
      event.preventDefault();
      goNext();
    }
  };

  if (steps.length === 0) return null;

  const positionerClassName = [
    "tour-panel-positioner",
    panelLayout.kind === "welcome" ? "is-welcome" : "is-targeted",
    isDocked ? "is-docked" : "",
  ].filter(Boolean).join(" ");
  const positionerStyle = panelLayout.kind === "floating" ? panelLayout.style : undefined;

  return (
    <FeedbackPortal>
      <div className={`tour-layer is-${phase}`} data-tour-step={step.id}>
        <InteractionBlockers
          interaction={step.interaction}
          rect={targetState.interactionRect}
        />
        <Spotlight
          maskId={maskId}
          rect={targetState.rect}
          hasTarget={hasRequestedTarget}
          stepId={step.id}
        />

        <div
          ref={setPositionerRef}
          className={positionerClassName}
          style={positionerStyle as CSSProperties | undefined}
        >
          <section
            ref={panelRef}
            className="tour-panel"
            role="dialog"
            aria-modal={step.interaction === "blocked" ? true : undefined}
            aria-live="polite"
            aria-labelledby="tour-panel-title"
            aria-describedby="tour-panel-description"
            onKeyDown={handlePanelKeyDown}
          >
            <div
              key={step.id}
              className={`tour-panel__stage is-${stepDirection}`}
              data-tour-motion={stepDirection}
            >
              <header className="tour-panel__header">
                <div className="tour-panel__heading">
                  <span className="tour-panel__title-icon" aria-hidden="true">
                    {step.target ? <MapPinned size={22} /> : <Sparkles size={24} />}
                  </span>
                  <h2 id="tour-panel-title">{step.title}</h2>
                </div>
                <button
                  type="button"
                  className="tour-panel__icon-button"
                  aria-label={copy.exitAria}
                  title={copy.exitAria}
                  onClick={() => requestFinish("skipped")}
                >
                  <X size={18} />
                </button>
              </header>

              <div className="tour-panel__body">
                <p id="tour-panel-description" className="tour-panel__description">
                  {step.description}
                </p>

                {step.features ? <FeatureList features={step.features} /> : null}
                {step.bullets ? (
                  <ul className="tour-panel__bullets">
                    {step.bullets.map((bullet) => (
                      <li key={bullet}>
                        <CheckCircle2 size={16} aria-hidden="true" />
                        <span>{bullet}</span>
                      </li>
                    ))}
                  </ul>
                ) : null}

                {isTargetPending ? (
                  <div className="tour-panel__pending" role="status">
                    <LoaderCircle size={17} aria-hidden="true" />
                    <span>{copy.locatingTarget}</span>
                  </div>
                ) : null}

                {isTargetUnavailable ? (
                  <div className="tour-panel__target-unavailable" role="status">
                    <RefreshCw size={17} aria-hidden="true" />
                    <div>
                      <strong>{copy.targetUnavailableTitle}</strong>
                      <span>
                        {isAwaitingRouteChange
                          ? copy.targetUnavailableRouteHint
                          : copy.targetUnavailableSkipHint}
                      </span>
                    </div>
                    <button type="button" onClick={targetState.retry}>{copy.relocate}</button>
                  </div>
                ) : null}

                {step.callout ? (
                  <div className="tour-panel__callout">
                    <Lightbulb size={18} aria-hidden="true" />
                    <strong>{step.callout}</strong>
                  </div>
                ) : null}
              </div>

              <footer className="tour-panel__footer">
                <strong className="tour-panel__counter">{stepIndex + 1} / {steps.length}</strong>
                <div className="tour-panel__navigation" aria-label={copy.navigationAria}>
                  <button
                    type="button"
                    className="tour-panel__icon-button"
                    aria-label={copy.previous}
                    title={copy.previous}
                    disabled={!canGoPrevious}
                    onClick={goPrevious}
                  >
                    <ArrowLeft size={17} />
                  </button>
                  <button
                    type="button"
                    className="tour-panel__icon-button"
                    aria-label={copy.next}
                    title={copy.next}
                    disabled={step.advance.kind !== "controls" || isTargetPending}
                    onClick={goNext}
                  >
                    <ArrowRight size={17} />
                  </button>
                </div>
                <button
                  type="button"
                  className="tour-panel__exit-button"
                  onClick={() => requestFinish("skipped")}
                >
                  {copy.exit}
                </button>
                {isAwaitingRouteChange ? (
                  <span className="tour-panel__interaction-hint" role="status">
                    <MousePointerClick size={16} aria-hidden="true" />
                    {step.primaryLabel}
                  </span>
                ) : (
                  <button
                    ref={primaryActionRef}
                    type="button"
                    className="tour-panel__primary-button"
                    disabled={isTargetPending}
                    onClick={goNext}
                  >
                    {stepIndex === 0 ? <Rocket size={17} /> : step.advance.kind === "terminal" ? <Check size={17} /> : null}
                    <span>{isTargetUnavailable ? copy.skipStep : step.primaryLabel || copy.emptyStepPrimary}</span>
                    {stepIndex > 0 && step.advance.kind === "controls" ? <ArrowRight size={17} /> : null}
                  </button>
                )}
              </footer>
            </div>
          </section>
        </div>
      </div>
    </FeedbackPortal>
  );
}

function InteractionBlockers({
  interaction,
  rect,
}: {
  interaction: TourStep["interaction"];
  rect: ReturnType<typeof useTourTarget>["interactionRect"];
}) {
  if (interaction !== "target-only" || !rect) {
    return <div className="tour-layer__blocker is-full" aria-hidden="true" />;
  }

  return (
    <div className="tour-layer__blockers" aria-hidden="true">
      <div className="tour-layer__blocker is-top" style={{ height: rect.top }} />
      <div
        className="tour-layer__blocker is-left"
        style={{ top: rect.top, width: rect.left, height: rect.height }}
      />
      <div
        className="tour-layer__blocker is-right"
        style={{ top: rect.top, left: rect.right, height: rect.height }}
      />
      <div className="tour-layer__blocker is-bottom" style={{ top: rect.bottom }} />
    </div>
  );
}

function Spotlight({
  maskId,
  rect,
  hasTarget,
  stepId,
}: {
  maskId: string;
  rect: ReturnType<typeof useTourTarget>["rect"];
  hasTarget: boolean;
  stepId: string;
}) {
  const [visualTarget, setVisualTarget] = useState({ stepId, rect });

  useEffect(() => {
    setVisualTarget((current) => {
      if (current.stepId !== stepId) return { stepId, rect };
      return rect ? { stepId, rect } : current;
    });
  }, [rect, stepId]);

  const visualRect = visualTarget.stepId === stepId ? visualTarget.rect : null;
  const isVisible = Boolean(rect || (hasTarget && visualRect));
  const geometryStyle = visualRect ? getSpotlightGeometryStyle(visualRect) : undefined;

  return (
    <>
      <svg className="tour-spotlight" width="100%" height="100%" aria-hidden="true">
        <defs>
          <mask id={maskId} maskUnits="userSpaceOnUse">
            <rect width="100%" height="100%" fill="white" />
            {visualRect ? (
              <rect
                className={`tour-spotlight__cutout${isVisible ? " is-visible" : ""}`}
                x={visualRect.left}
                y={visualRect.top}
                width={visualRect.width}
                height={visualRect.height}
                rx="10"
                fill="black"
                style={geometryStyle}
              />
            ) : null}
          </mask>
        </defs>
        <rect width="100%" height="100%" fill="rgba(2, 6, 23, 0.72)" mask={`url(#${maskId})`} />
      </svg>
      {visualRect ? (
        <div
          className={`tour-spotlight__ring${isVisible ? " is-visible" : ""}`}
          style={{
            top: visualRect.top,
            left: visualRect.left,
            width: visualRect.width,
            height: visualRect.height,
          }}
          aria-hidden="true"
        />
      ) : null}
    </>
  );
}

function useTourPanelRelocation(
  positionerRef: RefObject<HTMLDivElement | null>,
  panelRef: RefObject<HTMLElement | null>,
  enabled: boolean,
) {
  const previousLayoutRectRef = useRef<DOMRect | null>(null);
  const animationRef = useRef<Animation | null>(null);

  useLayoutEffect(() => {
    const positioner = positionerRef.current;
    const panel = panelRef.current;
    if (!positioner || !panel) return;

    const nextRect = positioner.getBoundingClientRect();
    const runningAnimation = animationRef.current;
    // Floating UI snaps the outer positioner; the inner panel follows from its last visual rect.
    const previousRect = runningAnimation?.playState === "running"
      ? panel.getBoundingClientRect()
      : previousLayoutRectRef.current;

    runningAnimation?.cancel();
    animationRef.current = null;
    previousLayoutRectRef.current = nextRect;

    if (!enabled || prefersReducedMotion() || !previousRect) return;

    const deltaX = previousRect.left - nextRect.left;
    const deltaY = previousRect.top - nextRect.top;
    if (Math.abs(deltaX) < 0.5 && Math.abs(deltaY) < 0.5) return;

    const animation = panel.animate(
      [
        { transform: `translate3d(${deltaX}px, ${deltaY}px, 0)` },
        { transform: "translate3d(0, 0, 0)" },
      ],
      {
        duration: TOUR_PANEL_RELOCATION_MS,
        easing: "cubic-bezier(0.2, 0.7, 0, 1)",
        fill: "both",
      },
    );
    animationRef.current = animation;
    void animation.finished.then(
      () => {
        if (animationRef.current !== animation) return;
        animation.cancel();
        animationRef.current = null;
      },
      () => undefined,
    );
  });

  useEffect(() => () => animationRef.current?.cancel(), []);
}

function useStableTourPanelLayout({
  stepId,
  hasRequestedTarget,
  targetRect,
  shouldDock,
  floatingX,
  floatingY,
  floatingStyle,
  floatingPositioned,
}: {
  stepId: string;
  hasRequestedTarget: boolean;
  targetRect: ReturnType<typeof useTourTarget>["rect"];
  shouldDock: boolean;
  floatingX: number | null;
  floatingY: number | null;
  floatingStyle: CSSProperties;
  floatingPositioned: boolean;
}) {
  const [layout, setLayout] = useState<StableTourPanelLayout>(() => (
    hasRequestedTarget
      ? { kind: "docked", stepId }
      : { kind: "welcome", stepId }
  ));

  useLayoutEffect(() => {
    if (!hasRequestedTarget) {
      setLayout((current) => current.kind === "welcome" && current.stepId === stepId
        ? current
        : { kind: "welcome", stepId });
      return;
    }

    // Keep the last stable layout while the next target is being resolved.
    // This prevents the positioner from briefly snapping to a fallback location.
    if (!targetRect) return;

    if (shouldDock) {
      setLayout((current) => current.kind === "docked" && current.stepId === stepId
        ? current
        : { kind: "docked", stepId });
      return;
    }

    if (floatingX === null || floatingY === null || !floatingPositioned) return;
    setLayout((current) => {
      if (
        current.kind === "floating"
        && current.stepId === stepId
        && Math.abs(current.x - floatingX) < 0.25
        && Math.abs(current.y - floatingY) < 0.25
      ) {
        return current;
      }

      return {
        kind: "floating",
        stepId,
        x: floatingX,
        y: floatingY,
        style: { ...floatingStyle },
      };
    });
  }, [
    floatingStyle,
    floatingPositioned,
    floatingX,
    floatingY,
    hasRequestedTarget,
    shouldDock,
    stepId,
    targetRect,
  ]);

  return layout;
}

function getSpotlightGeometryStyle(
  rect: NonNullable<ReturnType<typeof useTourTarget>["rect"]>,
): CSSProperties {
  return {
    "--tour-spotlight-x": `${rect.left}px`,
    "--tour-spotlight-y": `${rect.top}px`,
    "--tour-spotlight-width": `${rect.width}px`,
    "--tour-spotlight-height": `${rect.height}px`,
  } as CSSProperties;
}

function FeatureList({ features }: { features: NonNullable<TourStep["features"]> }) {
  return (
    <ul className="tour-panel__features">
      {features.map((feature) => {
        const Icon = featureIcon(feature.icon);
        return (
          <li key={feature.title}>
            <Icon size={19} aria-hidden="true" />
            <div>
              <strong>{feature.title}</strong>
              <span>{feature.description}</span>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

function featureIcon(icon: TourFeatureIcon) {
  switch (icon) {
    case "shield":
      return ShieldCheck;
    case "layers":
      return Layers3;
    case "profiles":
      return UserRoundCog;
    case "backup":
      return ArchiveRestore;
  }
}

function getTourTransitionMillis() {
  return prefersReducedMotion() ? 1 : TOUR_TRANSITION_MS;
}

function prefersReducedMotion() {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function useViewportSize() {
  const [size, setSize] = useState(readViewportSize);

  useEffect(() => {
    const updateSize = () => {
      const next = readViewportSize();
      setSize((previous) => previous.width === next.width && previous.height === next.height
        ? previous
        : next);
    };

    window.addEventListener("resize", updateSize);
    window.visualViewport?.addEventListener("resize", updateSize);
    return () => {
      window.removeEventListener("resize", updateSize);
      window.visualViewport?.removeEventListener("resize", updateSize);
    };
  }, []);

  return size;
}

function readViewportSize() {
  if (typeof window === "undefined") return { width: 0, height: 0 };
  return { width: window.innerWidth, height: window.innerHeight };
}
