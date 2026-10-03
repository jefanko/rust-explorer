import { describe, it, expect } from "vitest";

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
});
