import type {
  GamePrerequisiteDecision,
  GamePrerequisiteDecisionCode,
} from "./modInstallPlanTypes";
import type { ModLifecycleCopy } from "./modLifecycleCopy";

// 文案收敛在 modLifecycleCopy.prerequisite；这里只保留取词入口，
// codes 表按语言各自穷尽（Record<GamePrerequisiteDecisionCode, string>）。

export function getPrerequisiteDecisionCodeLabel(
  code: GamePrerequisiteDecisionCode,
  prerequisite: ModLifecycleCopy["prerequisite"],
) {
  return prerequisite.codes[code];
}

export function getPrerequisiteDecisionMessage(
  decision: GamePrerequisiteDecision,
  prerequisite: ModLifecycleCopy["prerequisite"],
) {
  switch (decision.status) {
    case "ready":
      return prerequisite.ready;
    case "warning":
      return prerequisite.warning;
    case "blocked":
      return prerequisite.blocked;
  }
}
