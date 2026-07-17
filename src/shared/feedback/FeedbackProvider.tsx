import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { FeedbackToast } from "./FeedbackToast";
import { TaskNotice, type TaskNoticeProps } from "./TaskNotice";
import { TaskNoticeViewport } from "./TaskNoticeViewport";
import { ToastViewport } from "./ToastViewport";
import { dismissFeedbackToast, enqueueFeedbackToast, type FeedbackToastInput, type FeedbackToastItem } from "./feedbackToastState";
import "./feedback.css";

const FeedbackHostContext = createContext<HTMLElement | null | undefined>(undefined);
type FeedbackTaskNoticeInput = Pick<TaskNoticeProps, "taskId" | "title" | "message" | "tone">;
const FeedbackActionsContext = createContext<{
  pushToast: (input: FeedbackToastInput) => void;
  dismissToast: (id: string) => void;
  showTaskNotice: (input: FeedbackTaskNoticeInput) => void;
  dismissTaskNotice: (taskId: string) => void;
} | null>(null);

type FeedbackProviderProps = {
  children: ReactNode;
};

export function FeedbackProvider({ children }: FeedbackProviderProps) {
  const [host, setHost] = useState<HTMLElement | null>(null);
  const [toasts, setToasts] = useState<FeedbackToastItem[]>([]);
  const [taskNotices, setTaskNotices] = useState<FeedbackTaskNoticeInput[]>([]);
  const sequenceRef = useRef(0);

  const pushToast = useCallback((input: FeedbackToastInput) => {
    sequenceRef.current += 1;
    setToasts((queue) => enqueueFeedbackToast(queue, input, sequenceRef.current));
  }, []);
  const dismissToast = useCallback((id: string) => setToasts((queue) => dismissFeedbackToast(queue, id)), []);
  const showTaskNotice = useCallback((input: FeedbackTaskNoticeInput) => {
    setTaskNotices((notices) => [...notices.filter((notice) => notice.taskId !== input.taskId), input]);
  }, []);
  const dismissTaskNotice = useCallback((taskId: string) => {
    setTaskNotices((notices) => notices.filter((notice) => notice.taskId !== taskId));
  }, []);

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

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape" || event.defaultPrevented || document.querySelector('[aria-modal="true"]')) return;
      setToasts((queue) => queue.length > 0 ? queue.slice(0, -1) : queue);
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  return (
    <FeedbackHostContext.Provider value={host}>
      <FeedbackActionsContext.Provider value={{ pushToast, dismissToast, showTaskNotice, dismissTaskNotice }}>
        {children}
        {taskNotices.length > 0 ? (
          <TaskNoticeViewport>
            {taskNotices.map((notice) => <TaskNotice key={notice.taskId} {...notice} />)}
          </TaskNoticeViewport>
        ) : null}
        {toasts.length > 0 ? (
          <ToastViewport>
            {toasts.map((toast) => <FeedbackToast key={toast.id} toast={toast} onDismiss={dismissToast} />)}
          </ToastViewport>
        ) : null}
      </FeedbackActionsContext.Provider>
    </FeedbackHostContext.Provider>
  );
}

export function useFeedback() {
  const value = useContext(FeedbackActionsContext);
  if (!value) throw new Error("useFeedback must be used within FeedbackProvider");
  return value;
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
