import { describe, it, expect } from "vitest";
import { formatBytes, formatFiletime } from "../lib/format";
import {
  getTabTitle,
  getBreadcrumbs,
  getParentPathUtf16,
  getParentPathString,
  getParentPath,
  sameWindowsPath,
} from "../lib/paths";
import { matchFilterQuery } from "../lib/filter";
import {
  tokensInRange,
  moveSelection,
  nextFocusIndex,
} from "../lib/selection";
import { validateIndexedQuery } from "../lib/searchQuery";
import { recycleDialogText, recycleTargetLabel } from "../lib/recycle";
import { createInitialTab } from "../state/tabs";

describe("Production Formatting Helpers", () => {
  it("correctly formats byte sizes across all standard units", () => {
    expect(formatBytes(null)).toBe("");
    expect(formatBytes(undefined)).toBe("");
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1 KB");
    expect(formatBytes(1048576)).toBe("1 MB");
    expect(formatBytes(1073741824)).toBe("1 GB");
    expect(formatBytes(1099511627776)).toBe("1 TB");
  });

  it("formats Windows FILETIME values into readable timestamps", () => {
    expect(formatFiletime(null)).toBe("");
    expect(formatFiletime(undefined)).toBe("");
    expect(formatFiletime(0)).toBe("");
    // A known valid Windows FILETIME: 133722240000000000 (~October 2024)
    const formatted = formatFiletime(133722240000000000);
    expect(formatted).not.toBe("");
    expect(typeof formatted).toBe("string");
  });
});

describe("Production Path Utilities", () => {
  it("extracts clean and readable tab titles from various Windows paths", () => {
    expect(getTabTitle("")).toBe("New Tab");
    expect(getTabTitle("C:\\")).toBe("C:");
    expect(getTabTitle("C:\\Users\\Default\\Documents")).toBe("Documents");
    expect(getTabTitle("D:\\dev\\rust-explorer\\")).toBe("rust-explorer");
    expect(getTabTitle("\\\\Server\\Share\\Subfolder")).toBe("Subfolder");
    expect(getTabTitle("\\\\?\\C:\\MyFolder")).toBe("MyFolder");
  });

  it("splits Windows paths into breadcrumb segments correctly", () => {
    const crumbs = getBreadcrumbs("C:\\Users\\Default\\Documents");
    expect(crumbs).toHaveLength(4);
    expect(crumbs[0]).toEqual({ label: "C:", fullPath: "C:\\" });
    expect(crumbs[1]).toEqual({ label: "Users", fullPath: "C:\\Users" });
    expect(crumbs[2]).toEqual({ label: "Default", fullPath: "C:\\Users\\Default" });
    expect(crumbs[3]).toEqual({ label: "Documents", fullPath: "C:\\Users\\Default\\Documents" });
  });

  it("compares Windows paths case-insensitively with sameWindowsPath", () => {
    const toUnits = (s: string) => Array.from(s).map((c) => c.charCodeAt(0));
    expect(sameWindowsPath(toUnits("C:\\Dev\\App"), toUnits("c:\\dev\\app"))).toBe(true);
    expect(sameWindowsPath(toUnits("C:\\Dev\\App"), toUnits("C:\\Dev\\Other"))).toBe(false);
    expect(sameWindowsPath(toUnits("C:\\Dev"), toUnits("C:\\Dev\\App"))).toBe(false);
  });

  it("computes parent folder correctly for extended, DOS, and UNC paths without stripping drive root slash", () => {
    // Extended drive paths
    expect(getParentPathString("\\\\?\\D:\\dev")).toBe("\\\\?\\D:\\");
    expect(getParentPathString("\\\\?\\D:\\dev\\rust-explorer")).toBe("\\\\?\\D:\\dev");
    expect(getParentPathString("\\\\?\\D:\\")).toBeNull();
    expect(getParentPathString("\\\\?\\D:")).toBeNull();

    // Standard DOS drive paths
    expect(getParentPathString("D:\\dev")).toBe("D:\\");
    expect(getParentPathString("D:\\dev\\rust-explorer")).toBe("D:\\dev");
    expect(getParentPathString("D:\\")).toBeNull();
    expect(getParentPathString("D:")).toBeNull();

    // UNC paths
    expect(getParentPathString("\\\\server\\share\\sub")).toBe("\\\\server\\share");
    expect(getParentPathString("\\\\server\\share")).toBeNull();
    expect(getParentPathString("\\\\?\\UNC\\server\\share\\sub")).toBe("\\\\?\\UNC\\server\\share");
    expect(getParentPathString("\\\\?\\UNC\\server\\share")).toBeNull();

    // UTF-16 code units helper
    const toUnits = (s: string) => Array.from(s).map((c) => c.charCodeAt(0));
    const fromUnits = (u: number[] | null) => (u ? String.fromCharCode(...u) : null);

    expect(fromUnits(getParentPathUtf16(toUnits("\\\\?\\D:\\dev")))).toBe("\\\\?\\D:\\");
    expect(getParentPathUtf16(toUnits("\\\\?\\D:\\"))).toBeNull();

    // Unified getParentPath
    const parentDev = getParentPath(toUnits("\\\\?\\D:\\dev"), "D:\\dev");
    expect(parentDev).not.toBeNull();
    expect(fromUnits(parentDev!.utf16!)).toBe("\\\\?\\D:\\");

    const parentRoot = getParentPath(toUnits("\\\\?\\D:\\"), "D:\\");
    expect(parentRoot).toBeNull();
  });
});

describe("Production Filter & Query Validation", () => {
  it("filters folder entries using matchFilterQuery with extensions, globs, and types", () => {
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

  it("validates indexed search queries using production validateIndexedQuery", () => {
    expect(validateIndexedQuery("").valid).toBe(false);
    expect(validateIndexedQuery("a")).toEqual({
      valid: false,
      error: "Use at least 3 characters for indexed search",
    });
    expect(validateIndexedQuery("ab")).toEqual({
      valid: false,
      error: "Use at least 3 characters for indexed search",
    });
    expect(validateIndexedQuery("abc").valid).toBe(true);
    expect(validateIndexedQuery("invoice").valid).toBe(true);
    expect(validateIndexedQuery("ext:pdf").valid).toBe(true);
    expect(validateIndexedQuery("*.exe").valid).toBe(true);
    expect(validateIndexedQuery(".exe").valid).toBe(true);
    expect(validateIndexedQuery("type:folder").valid).toBe(true);
    expect(validateIndexedQuery("a ext:pdf").valid).toBe(true);
    expect(validateIndexedQuery("a *.exe").valid).toBe(true);
  });
});

describe("Production Selection Helpers", () => {
  const entries = [
    { token: "t0" },
    { token: "t1" },
    { token: "t2" },
    { token: "t3" },
    { token: "t4" },
    { token: "t5" },
  ];

  it("tokensInRange collects tokens correctly irrespective of order", () => {
    const range1 = tokensInRange(entries, 1, 3);
    expect(Array.from(range1)).toEqual(["t1", "t2", "t3"]);

    const range2 = tokensInRange(entries, 4, 2);
    expect(Array.from(range2)).toEqual(["t2", "t3", "t4"]);
  });

  it("moveSelection calculates single selection and shift-range extensions", () => {
    // Single click / arrow
    const single = moveSelection(entries, 1, 3, false);
    expect(single).toEqual({
      focusedIndex: 3,
      anchorIndex: 3,
      selectedTokens: new Set(["t3"]),
    });

    // Shift range extension
    const extended = moveSelection(entries, 1, 3, true);
    expect(extended?.focusedIndex).toBe(3);
    expect(extended?.anchorIndex).toBe(1);
    expect(Array.from(extended?.selectedTokens ?? [])).toEqual(["t1", "t2", "t3"]);
  });

  it("nextFocusIndex handles boundary clamping and empty collections", () => {
    expect(nextFocusIndex(-1, 1, 5)).toBe(0);
    expect(nextFocusIndex(-1, -1, 5)).toBe(4);
    expect(nextFocusIndex(2, 1, 5)).toBe(3);
    expect(nextFocusIndex(4, 1, 5)).toBe(4);
    expect(nextFocusIndex(0, -1, 5)).toBe(0);
    expect(nextFocusIndex(-1, 1, 0)).toBe(-1);
  });
});

describe("Production Recycle Dialog Formatting", () => {
  it("formats confirmation dialog text accurately for single and multiple items", () => {
    const single = recycleDialogText(["report.pdf"]);
    expect(single.title).toBe('Recycle "report.pdf"?');
    expect(single.value).toBe("report.pdf");

    const multi = recycleDialogText(["a.txt", "b.txt", "c.txt"]);
    expect(multi.title).toBe("Recycle 3 items?");
    expect(multi.value).toBe("3 items");

    expect(recycleTargetLabel("report.pdf", 1)).toBe('"report.pdf"');
    expect(recycleTargetLabel("3 items", 3)).toBe("3 items");
  });
});

describe("Production Tab Factory", () => {
  it("initializes tab state cleanly with defaults", () => {
    const tab = createInitialTab("test_tab", "C:\\Users");
    expect(tab.id).toBe("test_tab");
    expect(tab.title).toBe("Users");
    expect(tab.path).toBe("C:\\Users");
    expect(tab.addressInput).toBe("C:\\Users");
    expect(tab.loading).toBe(true);
    expect(tab.history).toHaveLength(1);
    expect(tab.sortColumn).toBe("name");
    expect(tab.sortDirection).toBe("ascending");
    expect(tab.selectedTokens.size).toBe(0);
  });
});
