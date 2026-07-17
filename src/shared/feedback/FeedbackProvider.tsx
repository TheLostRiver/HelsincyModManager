import {
  createContext,
  useContext,
  useLayoutEffect,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import "./feedback.css";

const FeedbackHostContext = createContext<HTMLElement | null | undefined>(undefined);

type FeedbackProviderProps = {
  children: ReactNode;
};

export function FeedbackProvider({ children }: FeedbackProviderProps) {
  const [host, setHost] = useState<HTMLElement | null>(null);

  useLayoutEffect(() => {
    const nextHost = document.createElement("div");
    nextHost.className = "feedback-host";
    nextHost.dataset.feedbackHost = "true";
    document.body.appendChild(nextHost);
    setHost(nextHost);

    return () => {
      nextHost.remove();
    };
  }, []);

  return <FeedbackHostContext.Provider value={host}>{children}</FeedbackHostContext.Provider>;
}

type FeedbackPortalProps = {
  children: ReactNode;
};

export function FeedbackPortal({ children }: FeedbackPortalProps) {
  const host = useContext(FeedbackHostContext);

  if (host === undefined) {
    throw new Error("FeedbackPortal must be used within FeedbackProvider");
  }

  return host ? createPortal(children, host) : null;
}
