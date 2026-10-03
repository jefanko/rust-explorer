import { invoke } from "@tauri-apps/api/core";
import {
  BootstrapData,
  DirectoryPage,
  NavigationResponse,
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
};
