import type { PreviewImage } from "./modPreviewImageTypes";

export type ModInstallStatus = "installed" | "disabled" | "conflict";

export type ModLibraryItem = {
  id: string;
  name: string;
  author?: string;
  versionLabel?: string;
  sizeLabel: string;
  status: ModInstallStatus;
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
