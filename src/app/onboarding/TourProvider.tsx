import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useAppRoute } from "../routing/useAppRoute";
import { TourOverlay } from "../../shared/onboarding/TourOverlay";
import { saveTourOutcome, shouldAutoStartTour } from "../../shared/onboarding/tourStorage";
import type { TourDefinition, TourOutcome } from "../../shared/onboarding/tourTypes";
import { buildOnboardingTour } from "./firstRunTour";
import { TourContext } from "./TourContext";

type TourProviderProps = {
  children: ReactNode;
};

export function TourProvider({ children }: TourProviderProps) {
  const { currentRoute } = useAppRoute();
  const autoStartCheckedRef = useRef(false);
  const activatedTargetStepIdRef = useRef<string | null>(null);
  const [activeTour, setActiveTour] = useState<TourDefinition | null>(null);
  const [stepIndex, setStepIndex] = useState(0);
  const activeStep = activeTour?.steps[stepIndex];

  useEffect(() => {
    if (autoStartCheckedRef.current || currentRoute.id !== "dashboard") return undefined;

    const firstRunTour = buildOnboardingTour(currentRoute.id, { includeWelcome: true });
    const storage = getLocalStorage();
    if (!shouldAutoStartTour(firstRunTour, storage)) {
      autoStartCheckedRef.current = true;
      return undefined;
    }

    const frameId = window.requestAnimationFrame(() => {
      if (autoStartCheckedRef.current) return;
      autoStartCheckedRef.current = true;
      setStepIndex(0);
      setActiveTour(firstRunTour);
    });
    return () => window.cancelAnimationFrame(frameId);
  }, [currentRoute.id]);

  useEffect(() => {
    if (activeStep?.advance.kind !== "route-change") return undefined;
    if (activatedTargetStepIdRef.current !== activeStep.id) return undefined;
    if (activeStep.advance.expectedRouteId !== currentRoute.id) return undefined;

    const frameId = window.requestAnimationFrame(() => {
      activatedTargetStepIdRef.current = null;
      setStepIndex((current) => Math.min(current + 1, (activeTour?.steps.length ?? 1) - 1));
    });
    return () => window.cancelAnimationFrame(frameId);
  }, [activeStep, activeTour?.steps.length, currentRoute.id]);

  const changeStep = useCallback((index: number) => {
    activatedTargetStepIdRef.current = null;
    setStepIndex(index);
  }, []);

  const markTargetActivated = useCallback((stepId: string) => {
    activatedTargetStepIdRef.current = stepId;
  }, []);

  const startTour = useCallback(() => {
    autoStartCheckedRef.current = true;
    activatedTargetStepIdRef.current = null;
    setStepIndex(0);
    setActiveTour(buildOnboardingTour(currentRoute.id));
  }, [currentRoute.id]);

  const finishTour = useCallback((outcome: TourOutcome) => {
    if (activeTour) saveTourOutcome(activeTour, outcome, getLocalStorage());
    activatedTargetStepIdRef.current = null;
    setActiveTour(null);
    setStepIndex(0);
  }, [activeTour]);

  const contextValue = useMemo(() => ({
    isTourOpen: activeTour !== null,
    startTour,
  }), [activeTour, startTour]);

  return (
    <TourContext.Provider value={contextValue}>
      {children}
      {activeTour ? (
        <TourOverlay
          steps={activeTour.steps}
          stepIndex={stepIndex}
          onStepChange={changeStep}
          onTargetActivate={markTargetActivated}
          onFinish={finishTour}
        />
      ) : null}
    </TourContext.Provider>
  );
}

function getLocalStorage() {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}
