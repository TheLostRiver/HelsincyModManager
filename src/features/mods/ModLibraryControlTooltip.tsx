import { useId, useState, type KeyboardEvent, type ReactNode } from "react";
import "./ModLibraryControlTooltip.css";

type ModLibraryControlTooltipProps = {
  content?: string;
  describeControl?: boolean;
  children: (descriptionId: string | undefined) => ReactNode;
};

export function ModLibraryControlTooltip({
  content,
  describeControl = true,
  children,
}: ModLibraryControlTooltipProps) {
  const generatedId = useId();
  const [dismissed, setDismissed] = useState(false);
  const descriptionId = content && describeControl ? generatedId : undefined;

  const handleKeyDown = (event: KeyboardEvent<HTMLSpanElement>) => {
    if (!content || event.key !== "Escape") {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    setDismissed(true);
  };

  return (
    <span
      className="mod-library-control-tooltip"
      data-tooltip-dismissed={dismissed ? "true" : undefined}
      onPointerLeave={() => setDismissed(false)}
      onFocusCapture={() => setDismissed(false)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          setDismissed(false);
        }
      }}
      onKeyDown={handleKeyDown}
    >
      {children(descriptionId)}
      {content ? (
        <span
          id={descriptionId}
          className="mod-library-control-tooltip__bubble"
          role={describeControl ? "tooltip" : undefined}
          aria-hidden={describeControl ? undefined : true}
        >
          {content}
        </span>
      ) : null}
    </span>
  );
}
