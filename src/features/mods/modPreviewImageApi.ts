import { invoke } from "@tauri-apps/api/core";
import type { PreviewImage } from "./modPreviewImageTypes";

// Backend command already exists and is registered in
// docs/FRONTEND_BACKEND_CONTRACT.md: `get_mod_detail_preview_image` resolves the
// larger `preview-1024` derivation and returns null when the mod has no preview.
export function getModDetailPreviewImage(
  modId: string,
): Promise<PreviewImage | null> {
  return invoke("get_mod_detail_preview_image", { modId });
}
