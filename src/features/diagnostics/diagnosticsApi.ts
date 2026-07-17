import { invoke } from "@tauri-apps/api/core";
import type { DiagnosticsPageSnapshot, SupportDiagnosticsExport } from "./diagnosticsTypes";

export function getDiagnosticsPageSnapshot(): Promise<DiagnosticsPageSnapshot> {
  return invoke<DiagnosticsPageSnapshot>("get_diagnostics_page_snapshot");
}

export function exportSupportDiagnostics(): Promise<SupportDiagnosticsExport> {
  return invoke<SupportDiagnosticsExport>("export_support_diagnostics");
}
