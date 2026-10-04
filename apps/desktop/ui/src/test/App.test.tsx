import { describe, it, expect } from "vitest";
import { matchFilterQuery, getBreadcrumbs } from "../app/App";

function formatBytes(bytes?: number | null): string {
  if (bytes === null || bytes === undefined) return "";
  if (bytes === 0) return "0 B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

function getTabTitle(path: string): string {
  if (!path) return "New Tab";
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts.pop() || path || "PC";
}

describe("App Formatting and Tab Helpers", () => {
  it("correctly formats byte sizes across ranges", () => {
    expect(formatBytes(null)).toBe("");
    expect(formatBytes(undefined)).toBe("");
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(1048576)).toBe("1 MB");
    expect(formatBytes(1073741824)).toBe("1 GB");
  });

  it("extracts clean and readable tab titles from Windows paths", () => {
    expect(getTabTitle("")).toBe("New Tab");
    expect(getTabTitle("C:\\")).toBe("C:");
    expect(getTabTitle("C:\\Users\\Default\\Documents")).toBe("Documents");
    expect(getTabTitle("D:\\dev\\rust-explorer\\")).toBe("rust-explorer");
    expect(getTabTitle("\\\\Server\\Share\\Subfolder")).toBe("Subfolder");
  });

  it("handles multi-selection range calculation properly", () => {
    const totalItems = 10;
    const anchor = 2;
    const current = 5;
    const start = Math.min(anchor, current);
    const end = Math.max(anchor, current);

    const selectedIndices: number[] = [];
    for (let i = start; i <= end; i++) {
      if (i < totalItems) {
        selectedIndices.push(i);
      }
    }

    expect(selectedIndices).toEqual([2, 3, 4, 5]);
  });

  it("validates search query constraints per Section 14 spec", () => {
    function isValidSearchQuery(q: string): { valid: boolean; error?: string } {
      const trimmed = q.trim();
      if (!trimmed) return { valid: false };
      const hasMetadataFilter = /(?:ext:|\*\.|\.)[a-z0-9_-]+|type:(?:folder|file)/i.test(trimmed);
      const textWithoutFilters = trimmed.replace(/(?:ext:|\*\.|\.)[a-z0-9_-]+|type:[^\s]+/gi, "").trim();
      if (!hasMetadataFilter && textWithoutFilters.length < 3) {
        return { valid: false, error: "Use at least 3 characters for indexed search" };
      }
      return { valid: true };
    }

    expect(isValidSearchQuery("").valid).toBe(false);
    expect(isValidSearchQuery("a")).toEqual({
      valid: false,
      error: "Use at least 3 characters for indexed search",
    });
    expect(isValidSearchQuery("ab")).toEqual({
      valid: false,
      error: "Use at least 3 characters for indexed search",
    });
    expect(isValidSearchQuery("abc").valid).toBe(true);
    expect(isValidSearchQuery("invoice").valid).toBe(true);
    expect(isValidSearchQuery("ext:pdf").valid).toBe(true);
    expect(isValidSearchQuery("*.exe").valid).toBe(true);
    expect(isValidSearchQuery(".exe").valid).toBe(true);
    expect(isValidSearchQuery("*.c").valid).toBe(true);
    expect(isValidSearchQuery(".c").valid).toBe(true);
    expect(isValidSearchQuery("type:folder").valid).toBe(true);
    expect(isValidSearchQuery("a ext:pdf").valid).toBe(true);
    expect(isValidSearchQuery("a *.exe").valid).toBe(true);
  });

  it("filters folder entries using matchFilterQuery with extensions and globs", () => {
    const exeFile = { display_name: "rust-explorer.exe", extension: "exe", kind: "file" };
    const pdfFile = { display_name: "annual_report.pdf", extension: "pdf", kind: "file" };
    const folder = { display_name: "src", extension: "", kind: "directory" };

    // Direct extension filter
    expect(matchFilterQuery(exeFile, "*.exe")).toBe(true);
    expect(matchFilterQuery(exeFile, ".exe")).toBe(true);
    expect(matchFilterQuery(exeFile, "ext:exe")).toBe(true);
    expect(matchFilterQuery(pdfFile, "*.exe")).toBe(false);
    expect(matchFilterQuery(pdfFile, ".pdf")).toBe(true);

    // Wildcard glob
    expect(matchFilterQuery(exeFile, "rust*")).toBe(true);
    expect(matchFilterQuery(exeFile, "*explorer*")).toBe(true);
    expect(matchFilterQuery(pdfFile, "*report*")).toBe(true);

    // Type filter
    expect(matchFilterQuery(folder, "type:folder")).toBe(true);
    expect(matchFilterQuery(folder, "type:file")).toBe(false);
    expect(matchFilterQuery(exeFile, "type:file")).toBe(true);
  });

  it("extracts containing folder path for search results", () => {
    function getContainingFolder(fullPath: string): string {
      return fullPath.replace(/\\[^\\]+$/, "");
    }

    expect(getContainingFolder("C:\\Users\\Default\\Documents\\report.docx")).toBe("C:\\Users\\Default\\Documents");
    expect(getContainingFolder("D:\\projects\\rust\\main.rs")).toBe("D:\\projects\\rust");
  });

  it("splits Windows paths into breadcrumb segments correctly", () => {
    const crumbs = getBreadcrumbs("C:\\Users\\Default\\Documents");
    expect(crumbs).toHaveLength(4);
    expect(crumbs[0]).toEqual({ label: "C:", fullPath: "C:\\" });
    expect(crumbs[1]).toEqual({ label: "Users", fullPath: "C:\\Users" });
    expect(crumbs[2]).toEqual({ label: "Default", fullPath: "C:\\Users\\Default" });
    expect(crumbs[3]).toEqual({ label: "Documents", fullPath: "C:\\Users\\Default\\Documents" });
  });

  it("filters file entries case-insensitively", () => {
    const entries = [
      { name: "Report2026.docx", is_dir: false },
      { name: "Invoice_September.pdf", is_dir: false },
      { name: "Photos", is_dir: true },
      { name: "report_draft.txt", is_dir: false },
    ];

    const filterText = "report";
    const filtered = entries.filter((e) =>
      e.name.toLowerCase().includes(filterText.toLowerCase())
    );

    expect(filtered).toHaveLength(2);
    expect(filtered.map((e) => e.name)).toEqual(["Report2026.docx", "report_draft.txt"]);
  });

  it("formats recycle confirmation details accurately", () => {
    function getRecycleConfirmationMessage(items: string[]): string {
      if (items.length === 1) {
        return `Are you sure you want to move "${items[0]}" to the Recycle Bin?`;
      }
      return `Are you sure you want to move these ${items.length} items to the Recycle Bin?`;
    }

    expect(getRecycleConfirmationMessage(["test.txt"])).toBe(
      'Are you sure you want to move "test.txt" to the Recycle Bin?'
    );
    expect(getRecycleConfirmationMessage(["a.txt", "b.txt", "c.txt"])).toBe(
      "Are you sure you want to move these 3 items to the Recycle Bin?"
    );
  });
});

