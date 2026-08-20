import type { CategoryItem, CategoryRef } from "./modCategoryApi";
import type { UpdateModMetadataInput } from "./modMetadataApi";
import type { ModDetail, ModLibraryItem } from "./modLibraryTypes";

export type FormState = {
  displayName: string;
  author: string;
  version: string;
  description: string;
  nexusModId: string;
};

export const emptyForm: FormState = {
  displayName: "",
  author: "",
  version: "",
  description: "",
  nexusModId: "",
};

export type SaveModDetailChangesResult =
  | { status: "saved" }
  | { status: "metadata-failure" }
  | { status: "partial-category-failure" }
  | { status: "refresh-failure" };

type SaveModDetailChangesInput = {
  modId: string;
  metadata: Omit<UpdateModMetadataInput, "modId">;
  categoryIds: string[];
  categoriesReady: boolean;
  updateModMetadata: (input: UpdateModMetadataInput) => Promise<void>;
  setModCategories: (modId: string, categoryIds: string[]) => Promise<void>;
  onSaved: () => Promise<void> | void;
};

function versionFromLabel(versionLabel: string | undefined) {
  return versionLabel?.replace(/^v/i, "") ?? "";
}

/**
 * 把可空的后端字段填进受控输入框。
 * 后端 `Option<T>` 序列化后是 `null` 而非缺席字段，直接 String() 会得到 "null"，
 * 既污染表单又会被 parseNexusModId 判成"只能填写正整数"。
 * 这里用宽松相等一次性吃掉 null 与 undefined。
 */
export function formFieldFromOptional(value: string | number | null | undefined) {
  return value == null ? "" : String(value);
}

export function formFromDetail(detail: ModDetail | null, fallbackItem: ModLibraryItem | null | undefined): FormState {
  return {
    displayName: detail?.name ?? fallbackItem?.name ?? "",
    author: detail?.metadata.author ?? fallbackItem?.author ?? "",
    version: detail?.metadata.version ?? versionFromLabel(fallbackItem?.versionLabel),
    description: detail?.description ?? "",
    nexusModId: formFieldFromOptional(detail?.nexusModId),
  };
}

export function selectedIdsFromCategories(
  allCategories: CategoryItem[],
  assignedCategories: CategoryRef[],
  fallbackItem: ModLibraryItem | null | undefined,
  assignmentLoaded: boolean,
) {
  if (assignmentLoaded) {
    return new Set(assignedCategories.map((category) => category.id));
  }

  const fallbackNames = new Set((fallbackItem?.categoryLabels ?? []).map((label) => label.name));
  return new Set(allCategories.filter((category) => fallbackNames.has(category.name)).map((category) => category.id));
}

export function parseNexusModId(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return undefined;
  }
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }

  const parsed = Number.parseInt(trimmed, 10);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null;
}

async function refreshAfterSaved(onSaved: () => Promise<void> | void) {
  try {
    await onSaved();
    return true;
  } catch {
    return false;
  }
}

export async function saveModDetailChanges({
  modId,
  metadata,
  categoryIds,
  categoriesReady,
  updateModMetadata,
  setModCategories,
  onSaved,
}: SaveModDetailChangesInput): Promise<SaveModDetailChangesResult> {
  try {
    await updateModMetadata({ modId, ...metadata });
  } catch {
    return { status: "metadata-failure" };
  }

  if (categoriesReady) {
    try {
      await setModCategories(modId, categoryIds);
    } catch {
      await refreshAfterSaved(onSaved);
      return { status: "partial-category-failure" };
    }
  }

  const refreshed = await refreshAfterSaved(onSaved);
  return refreshed ? { status: "saved" } : { status: "refresh-failure" };
}
