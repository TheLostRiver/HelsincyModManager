import { invoke } from "@tauri-apps/api/core";
import type {
  GetModDetailInput,
  GetModRevisionsInput,
  ModDetail,
  ModLibraryItem,
  ModLibraryPage,
  ModRevisionList,
  QueryModLibraryInput,
} from "./modLibraryTypes";

export function getModLibrary(): Promise<ModLibraryItem[]> {
  return invoke<ModLibraryItem[]>("get_mod_library");
}

export function queryModLibrary(input: QueryModLibraryInput): Promise<ModLibraryPage> {
  return invoke<ModLibraryPage>("query_mod_library", {
    request: {
      ...(input.profileContext === undefined
        ? {}
        : {
            profileContext: {
              gameId: input.profileContext.gameId,
              profileId: input.profileContext.profileId,
            },
          }),
      search: input.search,
      filter: input.filter,
      sort: input.sort,
      page: input.page,
      pageSize: input.pageSize,
    },
  });
}

export function getModDetail(input: GetModDetailInput): Promise<ModDetail | null> {
  return invoke<ModDetail | null>("get_mod_detail", {
    modId: input.modId,
  });
}

export function getModRevisions(input: GetModRevisionsInput): Promise<ModRevisionList> {
  return invoke<ModRevisionList>("get_mod_revisions", { modId: input.modId });
}
