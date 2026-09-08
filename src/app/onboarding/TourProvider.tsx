import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { useAppRoute } from "../routing/useAppRoute";
import { TourOverlay } from "../../shared/onboarding/TourOverlay";
import { saveTourOutcome, shouldAutoStartTour } from "../../shared/onboarding/tourStorage";
import type { TourOutcome } from "../../shared/onboarding/tourTypes";
import type { AppRouteId } from "../routing/routeTypes";
import { buildOnboardingTour } from "./firstRunTour";
import { onboardingTourCopy } from "./onboardingTourCopy";
import { TourContext } from "./TourContext";
import { TourLanguagePicker } from "./TourLanguagePicker";

type TourProviderProps = {
  children: ReactNode;
};

export function TourProvider({ children }: TourProviderProps) {
  const { locale } = useI18n();
  const tourCopy = resolveCopy(onboardingTourCopy, locale);
  const { currentRoute } = useAppRoute();
  const autoStartCheckedRef = useRef(false);
  const activatedTargetStepIdRef = useRef<string | null>(null);
  // 只保存流程身份，步骤文案按当前语言投影，切换语言不重排路线或重置进度。
  const [activeRun, setActiveRun] = useState<{
    startRouteId: AppRouteId;
    includeWelcome: boolean;
  } | null>(null);
  const activeTour = useMemo(() => activeRun
    ? buildOnboardingTour(activeRun.startRouteId, tourCopy, activeRun)
    : null, [activeRun, tourCopy]);
  const [stepIndex, setStepIndex] = useState(0);
  const activeStep = activeTour?.steps[stepIndex];

  useEffect(() => {
    if (autoStartCheckedRef.current || currentRoute.id !== "dashboard") return undefined;

    const firstRunTour = buildOnboardingTour(currentRoute.id, tourCopy, { includeWelcome: true });
    const storage = getLocalStorage();
    if (!shouldAutoStartTour(firstRunTour, storage)) {
      autoStartCheckedRef.current = true;
      return undefined;
    }

    const frameId = window.requestAnimationFrame(() => {
      if (autoStartCheckedRef.current) return;
      autoStartCheckedRef.current = true;
      setStepIndex(0);
      setActiveRun({ startRouteId: currentRoute.id, includeWelcome: true });
    });
    return () => window.cancelAnimationFrame(frameId);
  }, [currentRoute.id, tourCopy]);

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
    setActiveRun({ startRouteId: currentRoute.id, includeWelcome: false });
  }, [currentRoute.id]);

  const finishTour = useCallback((outcome: TourOutcome) => {
    if (activeTour) saveTourOutcome(activeTour, outcome, getLocalStorage());
    activatedTargetStepIdRef.current = null;
    setActiveRun(null);
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
          renderStepContent={(step) => step.id === "language" ? <TourLanguagePicker /> : null}
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
