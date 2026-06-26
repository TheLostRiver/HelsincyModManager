import type { PreviewImage } from "./modPreviewImageTypes";
import type { InstallManifestStatus } from "./modInstallPlanTypes";

export type ModInstallStatus = InstallManifestStatus | "disabled" | "conflict";

export type ModInstallSummary = {
  status: InstallManifestStatus;
  managedFileCount: number;
  backupCount: number;
};

export type ModLibraryItem = {
  id: string;
  name: string;
  author?: string;
  versionLabel?: string;
  sizeLabel: string;
  status: ModInstallStatus;
  installSummary?: ModInstallSummary;
  categoryLabels: string[];
  posterFrom?: string;
  posterTo?: string;
  previewImage?: PreviewImage;
};

export type ModDetail = {
  id: string;
  name: string;
  packageId: string;
  previewImage: PreviewImage;
};

export type GetModDetailInput = {
  modId: string;
};
