import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  BootstrapData,
  DirectoryPage,
  NavigationResponse,
  AppSettings,
  ClipboardPayload,
  JobSummary,
  OperationPlan,
  SortColumn,
  SortDirection,
  WatchNotification,
  IndexedRoot,
  SearchResponse,
} from "./types";

export const client = {
  async bootstrap(): Promise<BootstrapData> {
    return invoke<BootstrapData>("bootstrap");
  },

  async navigate(path: string): Promise<NavigationResponse> {
    return invoke<NavigationResponse>("navigate", { path });
  },

  async listPage(
    folderToken: string,
    generation: number,
    offset: number,
    limit: number,
    sortColumn?: SortColumn,
    sortDirection?: SortDirection
  ): Promise<DirectoryPage> {
    return invoke<DirectoryPage>("list_page", {
      folderToken,
      generation,
      offset,
      limit,
      sortColumn,
      sortDirection,
    });
  },

  async refresh(folderToken: string): Promise<NavigationResponse> {
    return invoke<NavigationResponse>("refresh", { folderToken });
  },

  async openItem(
    folderToken: string,
    itemToken: string
  ): Promise<NavigationResponse | null> {
    return invoke<NavigationResponse | null>("open_item", {
      folderToken,
      itemToken,
    });
  },

  async showProperties(
    folderToken: string,
    itemToken?: string | null
  ): Promise<void> {
    return invoke<void>("show_properties", {
      folderToken,
      itemToken: itemToken || null,
    });
  },

  async openInExplorer(
    folderToken: string,
    itemToken?: string | null
  ): Promise<void> {
    return invoke<void>("open_in_explorer", {
      folderToken,
      itemToken: itemToken || null,
    });
  },

  async loadSettings(): Promise<AppSettings> {
    return invoke<AppSettings>("load_settings");
  },

  async saveSettings(settings: AppSettings): Promise<void> {
    return invoke<void>("save_settings", { settings });
  },

  async addFavorite(path: string): Promise<void> {
    return invoke<void>("add_favorite", { path });
  },

  async removeFavorite(path: string): Promise<void> {
    return invoke<void>("remove_favorite", { path });
  },

  async planCreateFolder(
    folderToken: string,
    name: string
  ): Promise<OperationPlan> {
    return invoke<OperationPlan>("plan_create_folder", {
      folderToken,
      name,
    });
  },

  async planRename(
    folderToken: string,
    itemToken: string,
    newName: string
  ): Promise<OperationPlan> {
    return invoke<OperationPlan>("plan_rename", {
      folderToken,
      itemToken,
      newName,
    });
  },

  async commitPlan(plan: OperationPlan): Promise<JobSummary> {
    return invoke<JobSummary>("commit_plan", { plan });
  },

  async createFolder(
    folderToken: string,
    name: string
  ): Promise<JobSummary> {
    return invoke<JobSummary>("create_folder", {
      folderToken,
      name,
    });
  },

  async renameItem(
    folderToken: string,
    itemToken: string,
    newName: string
  ): Promise<JobSummary> {
    return invoke<JobSummary>("rename_item", {
      folderToken,
      itemToken,
      newName,
    });
  },

  async listJobs(limit?: number): Promise<JobSummary[]> {
    return invoke<JobSummary[]>("list_jobs", { limit });
  },

  async planCopy(
    sources: string[],
    destination: string
  ): Promise<OperationPlan> {
    return invoke<OperationPlan>("plan_copy", { sources, destination });
  },

  async planMove(
    sources: string[],
    destination: string
  ): Promise<OperationPlan> {
    return invoke<OperationPlan>("plan_move", { sources, destination });
  },

  async planRecycle(sources: string[]): Promise<OperationPlan> {
    return invoke<OperationPlan>("plan_recycle", { sources });
  },

  async executeCopy(
    sources: string[],
    destination: string
  ): Promise<JobSummary> {
    return invoke<JobSummary>("execute_copy", { sources, destination });
  },

  async executeMove(
    sources: string[],
    destination: string
  ): Promise<JobSummary> {
    return invoke<JobSummary>("execute_move", { sources, destination });
  },

  async executeRecycle(sources: string[]): Promise<JobSummary> {
    return invoke<JobSummary>("execute_recycle", { sources });
  },

  async clipboardWrite(paths: string[], isCut: boolean): Promise<void> {
    return invoke<void>("clipboard_write", { paths, isCut });
  },

  async clipboardRead(): Promise<ClipboardPayload | null> {
    return invoke<ClipboardPayload | null>("clipboard_read");
  },

  async watchFolder(path: string, subscriberId: string): Promise<void> {
    return invoke<void>("watch_folder", { path, subscriberId });
  },

  async unwatchFolder(path: string, subscriberId: string): Promise<void> {
    return invoke<void>("unwatch_folder", { path, subscriberId });
  },

  async unwatchAll(subscriberId: string): Promise<void> {
    return invoke<void>("unwatch_all", { subscriberId });
  },

  async getWatchStatus(path: string): Promise<boolean> {
    return invoke<boolean>("get_watch_status", { path });
  },

  async onWatchNotification(
    callback: (notif: WatchNotification) => void
  ): Promise<UnlistenFn> {
    return listen<WatchNotification>("watch-notification", (event) => {
      callback(event.payload);
    });
  },

  async listIndexedRoots(): Promise<IndexedRoot[]> {
    return invoke<IndexedRoot[]>("list_indexed_roots");
  },

  async addIndexedRoot(path: string): Promise<IndexedRoot> {
    return invoke<IndexedRoot>("add_indexed_root", { path });
  },

  async removeIndexedRoot(rootId: string): Promise<void> {
    return invoke<void>("remove_indexed_root", { rootId });
  },

  async recrawlIndexedRoot(rootId: string): Promise<void> {
    return invoke<void>("recrawl_indexed_root", { rootId });
  },

  async searchIndexed(
    query: string,
    rootId?: string | null,
    page: number = 1,
    pageSize: number = 50
  ): Promise<SearchResponse> {
    return invoke<SearchResponse>("search_indexed", {
      query,
      rootId: rootId || null,
      page,
      pageSize,
    });
  },

  async openPath(path: string): Promise<NavigationResponse | null> {
    return invoke<NavigationResponse | null>("open_path", { path });
  },
};


