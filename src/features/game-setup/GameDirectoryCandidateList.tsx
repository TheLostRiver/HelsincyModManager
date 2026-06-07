import { CheckCircle2, CircleAlert, HardDrive } from "lucide-react";
import type { GameDirectoryCandidate } from "./gameSetupTypes";
import { messageForError } from "./gameSetupViewModel";
import "./GameDirectoryCandidateList.css";

type GameDirectoryCandidateListProps = {
  candidates: GameDirectoryCandidate[];
  isBusy: boolean;
  onCandidateSelected: (directory: string) => Promise<void>;
};

export function GameDirectoryCandidateList({
  candidates,
  isBusy,
  onCandidateSelected,
}: GameDirectoryCandidateListProps) {
  if (candidates.length === 0) {
    return null;
  }

  return (
    <section className="candidate-list" aria-label="Steam 候选目录">
      {candidates.map((candidate) => (
        <article className="candidate-item" key={candidate.directory}>
          <div className="candidate-icon" aria-hidden="true">
            {candidate.isValid ? <CheckCircle2 size={18} /> : <CircleAlert size={18} />}
          </div>
          <div className="candidate-content">
            <span className="candidate-source">
              <HardDrive size={14} />
              {candidate.sourceLabel}
            </span>
            <strong>{candidate.displayName}</strong>
            <p>{candidate.pathLabel}</p>
            {!candidate.isValid && candidate.errors.length > 0 ? (
              <small>{messageForError(candidate.errors[0])}</small>
            ) : null}
          </div>
          <button
            type="button"
            className="candidate-select"
            disabled={isBusy || !candidate.isValid}
            onClick={() => void onCandidateSelected(candidate.directory)}
          >
            使用此目录
          </button>
        </article>
      ))}
    </section>
  );
}
