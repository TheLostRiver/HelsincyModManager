import type {
  GamePrerequisiteDecision,
  GamePrerequisiteDecisionCode,
} from "./modInstallPlanTypes";

const prerequisiteCodeLabels: Record<GamePrerequisiteDecisionCode, string> = {
  game_not_configured: "尚未配置游戏目录",
  game_directory_invalid: "游戏目录校验失败",
  game_directory_not_writable: "游戏目录当前不可写，请关闭游戏与相关程序，或用管理员身份运行后重试",
  rules_unavailable: "前置规则不可用",
  rules_corrupted: "前置规则已损坏",
  storage_unavailable: "前置状态存储不可用",
  storage_corrupted: "前置状态存储已损坏",
  unsupported_game: "当前游戏不支持前置检查",
  missing_required_file: "缺少必要前置文件",
  signature_unverified: "前置文件签名无法验证",
  config_read_failed: "前置配置读取失败",
  config_invalid_json: "前置配置格式无效",
  config_field_mismatch: "前置配置未启用必要选项",
  prerequisite_decision_invalid: "前置检查结果无效",
};

export function getPrerequisiteDecisionCodeLabel(code: GamePrerequisiteDecisionCode) {
  return prerequisiteCodeLabels[code];
}

export function getPrerequisiteDecisionMessage(decision: GamePrerequisiteDecision) {
  switch (decision.status) {
    case "ready":
      return "前置检查通过。";
    case "warning":
      return "前置文件存在未验证项，确认来源可信后仍可继续。";
    case "blocked":
      return "前置检查未通过，后端已阻止写入。";
  }
}
