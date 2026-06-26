import { invoke } from "@tauri-apps/api/core";
import type { SupportDiagnosticsExport } from "./recoveryDiagnosticsTypes";

export function exportSupportDiagnostics(): Promise<SupportDiagnosticsExport> {
  return invoke<SupportDiagnosticsExport>("export_support_diagnostics");
}
