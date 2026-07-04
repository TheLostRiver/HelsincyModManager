import { invoke } from "@tauri-apps/api/core";
import type {
  ConfirmProfileSaveDirectoryCandidateInput,
  DiscoverProfileSaveDirectoriesInput,
  SaveDirectoryDiscoveryDto,
} from "./profileSaveDirectoryDiscoveryTypes";

export function discoverProfileSaveDirectories(
  input: DiscoverProfileSaveDirectoriesInput,
): Promise<SaveDirectoryDiscoveryDto> {
  return invoke<SaveDirectoryDiscoveryDto>("discover_profile_save_directories", input);
}

export function confirmProfileSaveDirectoryCandidate(
  input: ConfirmProfileSaveDirectoryCandidateInput,
): Promise<SaveDirectoryDiscoveryDto> {
  return invoke<SaveDirectoryDiscoveryDto>("confirm_profile_save_directory_candidate", {
    discoveryId: input.discoveryId,
    candidateId: input.candidateId,
  });
}
