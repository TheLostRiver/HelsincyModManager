// 「检查更新」偏好的持久化。与项目既有偏好一致，落 localStorage
// （后端不保存这些状态，见契约文档）。读写都吞掉异常：偏好存储失败
// 只该让用户回到默认体验，不该让「关于」页挂掉。

import { sanitizeUpdateCheckPreference } from "./updateCheckPolicy.ts";
import {
  DEFAULT_UPDATE_CHECK_PREFERENCE,
  type UpdateCheckPreference,
} from "./updateCheckTypes.ts";

const storageKey = "helsincy.updateCheckPreference";

type PersistedUpdateCheckPreference = {
  version: 1;
  preference: UpdateCheckPreference;
};

export function readUpdateCheckPreference(): UpdateCheckPreference {
  try {
    const rawValue = window.localStorage.getItem(storageKey);
    if (rawValue === null) {
      return DEFAULT_UPDATE_CHECK_PREFERENCE;
    }

    const parsedValue = JSON.parse(rawValue) as Partial<PersistedUpdateCheckPreference>;
    if (parsedValue?.version !== 1) {
      return DEFAULT_UPDATE_CHECK_PREFERENCE;
    }

    return sanitizeUpdateCheckPreference(parsedValue.preference);
  } catch {
    return DEFAULT_UPDATE_CHECK_PREFERENCE;
  }
}

export function writeUpdateCheckPreference(preference: UpdateCheckPreference) {
  try {
    const value: PersistedUpdateCheckPreference = { version: 1, preference };
    window.localStorage.setItem(storageKey, JSON.stringify(value));
  } catch {
    return;
  }
}
