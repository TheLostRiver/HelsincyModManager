export type RetargetFileDisposition =
  | "relocated" | "kept_in_place" | "package_companion"
  | "installed_attachment_retained" | "plugin_candidate" | "policy_excluded";

export type RetargetFileReason =
  | "target_mapping" | "original_target" | "texture_reference" | "unmapped_resource"
  | "package_resource" | "installed_attachment" | "plugin_not_included" | "executable_policy";

export type RetargetFilePreview = {
  fileId: string;
  sourceId: string | null;
  sourcePath: string;
  installedPath: string | null;
  targetPath: string | null;
  disposition: RetargetFileDisposition;
  reason: RetargetFileReason;
  change: "retained" | "replaced" | "added" | "stale" | null;
};
