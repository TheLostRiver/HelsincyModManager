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
      autoStartCheckedRef.current = true;
      setStepIndex(0);
      setActiveTour(firstRunTour);
    });
    return () => window.cancelAnimationFrame(frameId);
  }, [currentRoute.id]);

  useEffect(() => {
    if (activeStep?.advance.kind !== "route-change") return undefined;
    if (activeStep.advance.expectedRouteId !== currentRoute.id) return undefined;

    const frameId = window.requestAnimationFrame(() => {
      setStepIndex((current) => Math.min(current + 1, (activeTour?.steps.length ?? 1) - 1));
    });
    return () => window.cancelAnimationFrame(frameId);
  }, [activeStep, activeTour?.steps.length, currentRoute.id]);

  const startTour = useCallback(() => {
    setStepIndex(0);
    setActiveTour(buildOnboardingTour(currentRoute.id));
  }, [currentRoute.id]);

  const finishTour = useCallback((outcome: TourOutcome) => {
    if (activeTour) saveTourOutcome(activeTour, outcome, getLocalStorage());
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
          onStepChange={setStepIndex}
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
