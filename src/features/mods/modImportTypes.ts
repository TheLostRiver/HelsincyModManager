export type StartImportModTaskInput = {
  archivePath: string;
};

export type TaskStartedDto = {
  taskId: string;
  kind: "mod_import";
  status: "queued" | "running" | "completed" | "failed" | "cancelled";
};
