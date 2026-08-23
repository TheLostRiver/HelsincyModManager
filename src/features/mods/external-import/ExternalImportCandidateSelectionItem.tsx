import { LoaderCircle } from "lucide-react";
import { useId } from "react";
import { resolveCopy, useI18n } from "../../../shared/i18n";
import type { CategoryItem } from "../../categories/categoryApi";
import {
  externalImportCopy,
  type ExternalImportCopy,
} from "./externalImportCopy";
import type { ExternalImportPreviewCandidateViewModel } from "./externalImportPreviewModel";
import {
  canSelectExternalImportCandidateWithDecision,
  getRequiredExternalImportConflictResolution,
} from "./externalImportSelectionModel";
import type {
  ExternalImportConflictResolution,
  ExternalImportSelectionDecisionDto,
} from "./externalImportTypes";

type ExternalImportCandidateSelectionItemProps = {
  candidate: ExternalImportPreviewCandidateViewModel;
  categories: CategoryItem[];
  decision: ExternalImportSelectionDecisionDto;
  disabled: boolean;
  pending: boolean;
  onDecisionChange: (decision: ExternalImportSelectionDecisionDto) => void;
  onSelectedChange: (selected: boolean) => void;
};

function resolutionLabel(
  resolution: ExternalImportConflictResolution,
  candidate: ExternalImportCopy["candidate"],
) {
  return resolution === "keep_both"
    ? candidate.keepBothHint
    : candidate.ignoreInvalidHint;
}

export function ExternalImportCandidateSelectionItem({
  candidate,
  categories,
  decision,
  disabled,
  pending,
  onDecisionChange,
  onSelectedChange,
}: ExternalImportCandidateSelectionItemProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(externalImportCopy, locale).candidate;
  const checkboxId = useId();
  const resolutionId = useId();
  const categoryId = useId();
  const requiredResolution = getRequiredExternalImportConflictResolution(
    candidate.previewStatus,
  );
  const supported = requiredResolution !== "unsupported";
  const canSelect =
    supported &&
    canSelectExternalImportCandidateWithDecision(
      candidate.previewStatus,
      decision.conflictResolution,
    );

  return (
    <li className="external-import__candidate">
      <div className="external-import__candidate-selection">
        <input
          id={checkboxId}
          type="checkbox"
          checked={candidate.selected}
          disabled={disabled || pending || (!candidate.selected && !canSelect)}
          onChange={(event) => onSelectedChange(event.currentTarget.checked)}
        />
        <label htmlFor={checkboxId}>
          <span className="external-import__candidate-main">
            <strong>{candidate.title}</strong>
            {candidate.metadata.length > 0 ? (
              <span>{candidate.metadata.join(" · ")}</span>
            ) : null}
          </span>
        </label>
        {pending ? (
          <LoaderCircle
            className="external-import__spinner"
            size={16}
            aria-label={copy.savingSelection}
          />
        ) : null}
      </div>

      <div className="external-import__candidate-meta">
        <span>{candidate.fileCount}</span>
        <span>{candidate.totalBytes}</span>
      </div>

      <div className="external-import__candidate-statuses">
        <span className={`external-import__badge is-${candidate.statusTone}`}>
          {candidate.statusLabel}
        </span>
        {candidate.conflictLabel ? (
          <span className="external-import__badge is-neutral">
            {candidate.conflictLabel}
          </span>
        ) : null}
      </div>

      {requiredResolution !== null && requiredResolution !== "unsupported" ? (
        <div className="external-import__candidate-field">
          <label htmlFor={resolutionId}>{copy.conflictLabel}</label>
          <select
            id={resolutionId}
            value={decision.conflictResolution ?? ""}
            disabled={disabled || pending}
            onChange={(event) =>
              onDecisionChange({
                ...decision,
                conflictResolution:
                  event.currentTarget.value === requiredResolution
                    ? requiredResolution
                    : null,
              })
            }
          >
            <option value="">{copy.conflictPlaceholder}</option>
            <option value={requiredResolution}>
              {resolutionLabel(requiredResolution, copy)}
            </option>
          </select>
        </div>
      ) : null}

      {supported ? (
        <div className="external-import__candidate-field">
          <label htmlFor={categoryId}>{copy.categoryLabel}</label>
          <select
            id={categoryId}
            value={decision.categoryId ?? ""}
            disabled={disabled || pending}
            onChange={(event) =>
              onDecisionChange({
                ...decision,
                categoryId: event.currentTarget.value || null,
              })
            }
          >
            <option value="">{copy.categoryNone}</option>
            {categories.map((category) => (
              <option key={category.id} value={category.id}>
                {category.name}
              </option>
            ))}
          </select>
        </div>
      ) : (
        <p className="external-import__candidate-blocked">
          {copy.unselectable}
        </p>
      )}
    </li>
  );
}
