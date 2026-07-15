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

export type ModPackageMetadata = {
  version?: string;
  author?: string;
  category?: string;
  tags: string[];
  dependencies: string[];
};

export type ModDetail = {
  id: string;
  name: string;
  packageId: string;
  metadata: ModPackageMetadata;
  description?: string;
  nexusModId?: number;
  previewImage: PreviewImage;
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
