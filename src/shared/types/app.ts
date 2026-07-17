export type AppHealth =
  | "ok"
  | "app_log_event_rejected"
  | "app_log_retention_failed"
  | "app_log_write_failed"
  | "app_log_initialization_failed";

export type SetupStatus = "not_scanned" | "pending" | "ready";
