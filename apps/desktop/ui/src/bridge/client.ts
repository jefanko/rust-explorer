import { invoke } from "@tauri-apps/api/core";
import {
  BootstrapData,
  DirectoryPage,
  NavigationResponse,
  AppSettings,
  SortColumn,
  SortDirection,
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
};
