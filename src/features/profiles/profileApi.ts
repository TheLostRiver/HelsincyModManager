import { invoke } from "@tauri-apps/api/core";
import type { CreateProfileInput, Profile, UpdateProfileInput } from "./profileTypes";

export function listProfiles(): Promise<Profile[]> {
  return invoke<Profile[]>("list_profiles");
}

export function getActiveProfile(): Promise<Profile> {
  return invoke<Profile>("get_active_profile");
}

export function createProfile(input: CreateProfileInput): Promise<string> {
  return invoke<string>("create_profile", {
    name: input.name,
    description: input.description,
  });
}

export function updateProfile(input: UpdateProfileInput): Promise<void> {
  return invoke<void>("update_profile", {
    profileId: input.profileId,
    name: input.name,
    description: input.description,
  });
}

export function deleteProfile(profileId: string): Promise<void> {
  return invoke<void>("delete_profile", { profileId });
}

export function setActiveProfile(profileId: string): Promise<void> {
  return invoke<void>("set_active_profile", { profileId });
}
