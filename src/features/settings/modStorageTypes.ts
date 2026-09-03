// 直连 locales 而不是 shared/i18n barrel，且带 .ts 显式扩展：本模块会被 node --test 直接
// import（type stripping 不处理 JSX、不做扩展名推断），barrel 里的 I18nProvider.tsx 会拖垮它。
import { resolveCopy, type Locale } from "../../shared/i18n/locales.ts";
import {
  modStorageCopy,
  type ModStorageCopy,
  type ModStorageDegradedReason,
  type ModStorageErrorCode,
} from "./modStorageCopy.ts";

export type ModStorageRootSource = "default" | "configured";
export type ModStorageWritesFrozen = "none" | "migration" | "restart_required";

/** `get_mod_storage_settings` / `set_mod_storage_dir` 的返回（契约「Mod 存储目录」）。 */
export type ModStorageSettingsDto = {
  effectiveDir: string;
  defaultDir: string;
  configuredDir: string | null;
  source: ModStorageRootSource;
  /** 仅启动解析降级时出现（serde skip）。 */
  degradedReason?: ModStorageDegradedReason;
  degradedDetail?: string;
  libraryEmpty: boolean;
  restartRequired: boolean;
  writesFrozen: ModStorageWritesFrozen;
};

/** `validate_mod_storage_dir` 的返回；校验不通过不抛错，以 `code` 表达。 */
export type ModStorageDirValidationDto = {
  ok: boolean;
  code: string | null;
  exists: boolean;
  claimed: boolean;
};

const ROOT_SOURCES: readonly ModStorageRootSource[] = ["default", "configured"];
const WRITES_FROZEN: readonly ModStorageWritesFrozen[] = ["none", "migration", "restart_required"];
const DEGRADED_REASONS: readonly ModStorageDegradedReason[] = [
  "settings_unreadable",
  "configured_dir_invalid",
  "configured_dir_unavailable",
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/**
 * 后端 DTO 的形状守卫：字段缺失或枚举值不在契约内一律视为不可用，而不是让 UI 在
 * `undefined` 上做判断。降级两个字段是可选的（后端 skip_serializing_if）。
 */
export function isModStorageSettingsDto(value: unknown): value is ModStorageSettingsDto {
  if (!isRecord(value)) {
    return false;
  }
  return (
    typeof value.effectiveDir === "string" &&
    typeof value.defaultDir === "string" &&
    (value.configuredDir === null || typeof value.configuredDir === "string") &&
    ROOT_SOURCES.includes(value.source as ModStorageRootSource) &&
    (value.degradedReason === undefined ||
      DEGRADED_REASONS.includes(value.degradedReason as ModStorageDegradedReason)) &&
    (value.degradedDetail === undefined || typeof value.degradedDetail === "string") &&
    typeof value.libraryEmpty === "boolean" &&
    typeof value.restartRequired === "boolean" &&
    WRITES_FROZEN.includes(value.writesFrozen as ModStorageWritesFrozen)
  );
}

export function isModStorageDirValidationDto(value: unknown): value is ModStorageDirValidationDto {
  if (!isRecord(value)) {
    return false;
  }
  return (
    typeof value.ok === "boolean" &&
    (value.code === null || typeof value.code === "string") &&
    typeof value.exists === "boolean" &&
    typeof value.claimed === "boolean"
  );
}

/** 从 `CommandErrorDto` 取稳定码；形状不对就用调用方给的兜底码。 */
export function modStorageErrorCodeFrom(error: unknown, fallback: ModStorageErrorCode): string {
  if (isRecord(error) && typeof error.code === "string" && error.code.trim() !== "") {
    return error.code;
  }
  return fallback;
}

function isKnownErrorCode(
  code: string,
  errors: ModStorageCopy["errors"],
): code is ModStorageErrorCode {
  return code !== "unknown" && Object.prototype.hasOwnProperty.call(errors, code);
}

export function getModStorageErrorMessage(code: string, locale: Locale): string {
  const errors = resolveCopy(modStorageCopy, locale).errors;
  return isKnownErrorCode(code, errors) ? errors[code] : errors.unknown(code);
}

/**
 * 导入 / 删除入口的禁用原因：只按后端 `writesFrozen` 取词，不用 `restartRequired` 等字段复算
 * （契约：门禁事实一律来自后端，前端只投影）。
 */
export function getModStorageFreezeReason(
  writesFrozen: ModStorageWritesFrozen,
  locale: Locale,
): string | undefined {
  if (writesFrozen === "none") {
    return undefined;
  }
  return resolveCopy(modStorageCopy, locale).frozen[writesFrozen];
}

export function getModStorageDegradedMessage(
  reason: ModStorageDegradedReason,
  locale: Locale,
): string {
  return resolveCopy(modStorageCopy, locale).degraded[reason];
}
