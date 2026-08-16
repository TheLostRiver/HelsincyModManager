import { createContext, useContext } from "react";

export type TourContextValue = {
  isTourOpen: boolean;
  startTour: () => void;
};

export const TourContext = createContext<TourContextValue | null>(null);

export function useTour() {
  const context = useContext(TourContext);
  if (!context) {
    throw new Error("useTour must be used inside TourProvider.");
  }
  return context;
}
