import type { PreviewImage } from "./modPreviewImageTypes";
import type { InstallManifestStatus, InstallRecoveryIssueSummary, InstallRecoveryStatus } from "./modInstallPlanTypes";

export type ModInstallSummaryStatus = InstallManifestStatus | "rollback_required";
export type ModInstallStatus = ModInstallSummaryStatus | "disabled" | "conflict";

export type ModInstallSummary = {
  status: ModInstallSummaryStatus;
  managedFileCount: number;
  backupCount: number;
  recoveryStatus?: InstallRecoveryStatus;
  issueCount?: number;
  issues?: InstallRecoveryIssueSummary[];
};

export type CategoryLabel = {
  name: string;
  color?: string | null;
};

export type ModLibraryItem = {
  id: string;
  name: string;
  author?: string;
  versionLabel?: string;
  sizeLabel: string;
  status: ModInstallStatus;
  installSummary?: ModInstallSummary;
  categoryLabels: CategoryLabel[];
  posterFrom?: string;
  posterTo?: string;
  previewImage?: PreviewImage;
};

export type ModLibraryProfileContext = {
  gameId: string;
  profileId: string;
};

export type ModLibraryQueryFilter =
  | { kind: "all" }
  | { kind: "status"; status: InstallManifestStatus }
  | { kind: "category"; categoryId: string };

export type QueryModLibraryInput = {
  profileContext?: ModLibraryProfileContext;
  search: string;
  filter: ModLibraryQueryFilter;
  sort: "name_asc";
  page: number;
  pageSize: 12 | 24 | 48 | 96;
};

export type ModLibraryPage = {
  items: ModLibraryItem[];
  page: number;
  pageSize: 12 | 24 | 48 | 96;
  libraryTotal: number;
  matchingTotal: number;
};

export type ModPackageMetadata = {
  version?: string;
  author?: string;
  category?: string;
  tags: string[];
  dependencies: string[];
};

export type ModOriginKind = "imported" | "external_import" | "migrated_v1";

/** 脱敏来源摘要:只携带稳定 ID 与导入时间,后端保证不含任何私有摘要。 */
export type ModOrigin = {
  kind: ModOriginKind;
  adapterId: string | null;
  batchId: string | null;
  importedAtUnixMillis: number | null;
};

export type ModDetail = {
  id: string;
  name: string;
  packageId: string;
  metadata: ModPackageMetadata;
  description?: string;
  /**
   * 后端 `Option<u64>`（src-tauri/src/dto.rs 的 ModDetailDto）在 serde 下序列化为
   * `null` 而不是缺席字段，所以这里必须显式允许 null。
   * 曾经只声明 `?: number`，导致表单把 `null` 直接 String() 成 "null"。
   */
  nexusModId?: number | null;
  previewImage: PreviewImage;
  origin: ModOrigin;
};

export type GetModDetailInput = {
  modId: string;
};

export type GetModRevisionsInput = {
  modId: string;
};

export type ModRevisionSummary = {
  revisionId: string;
};

export type ModRevisionList = {
  modId: string;
  originRevisionId: string;
  displayRevisionId: string;
  revisions: ModRevisionSummary[];
};
