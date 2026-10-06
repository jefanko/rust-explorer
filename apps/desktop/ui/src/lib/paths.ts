export function getTabTitle(path: string): string {
  if (!path) return "New Tab";
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts.pop() || path || "PC";
}

export function getBreadcrumbs(path: string): { label: string; fullPath: string }[] {
  if (!path) return [];
  const parts = path.split(/[\\/]/).filter(Boolean);
  const crumbs: { label: string; fullPath: string }[] = [];
  let accumulated = "";

  for (let i = 0; i < parts.length; i++) {
    const part = parts[i];
    if (i === 0 && part.includes(":")) {
      accumulated = `${part}\\`;
    } else {
      accumulated = accumulated.endsWith("\\")
        ? `${accumulated}${part}`
        : `${accumulated}\\${part}`;
    }
    crumbs.push({ label: part, fullPath: accumulated });
  }
  return crumbs;
}

export function getParentPathUtf16(units: number[]): number[] | null {
  if (!units || units.length === 0) return null;

  // Trim any trailing slashes (e.g. D:\foo\ -> D:\foo)
  let end = units.length;
  while (end > 0 && (units[end - 1] === 92 || units[end - 1] === 47)) {
    end--;
  }
  if (end === 0) return null;

  const isLetter = (u: number) => (u >= 65 && u <= 90) || (u >= 97 && u <= 122);

  // Check for extended prefix: \\?\
  const isExtended =
    end >= 4 &&
    units[0] === 92 &&
    units[1] === 92 &&
    units[2] === 63 &&
    units[3] === 92;

  // Check for device prefix: \\.\
  const isDevice =
    end >= 4 &&
    units[0] === 92 &&
    units[1] === 92 &&
    units[2] === 46 &&
    units[3] === 92;

  // Check for \\?\UNC\
  const isExtendedUnc =
    isExtended &&
    end >= 8 &&
    (units[4] === 85 || units[4] === 117) &&
    (units[5] === 78 || units[5] === 110) &&
    (units[6] === 67 || units[6] === 99) &&
    units[7] === 92;

  // 1. Extended or Device Drive Root: \\?\C: or \\.\C:
  if ((isExtended || isDevice) && !isExtendedUnc && end >= 6 && isLetter(units[4]) && units[5] === 58) {
    if (end === 6) return null; // Already at \\?\C: (root)
    const lastSep = Math.max(
      units.slice(0, end).lastIndexOf(92),
      units.slice(0, end).lastIndexOf(47)
    );
    if (lastSep < 6) return null;
    // If last separator is index 6 (right after C:), parent is the drive root WITH trailing slash (\\?\C:\)
    if (lastSep === 6) {
      return units.slice(0, 7);
    }
    return units.slice(0, lastSep);
  }

  // 2. Standard DOS Drive Root: C:
  if (end >= 2 && isLetter(units[0]) && units[1] === 58) {
    if (end === 2) return null; // Already at C: (root)
    const lastSep = Math.max(
      units.slice(0, end).lastIndexOf(92),
      units.slice(0, end).lastIndexOf(47)
    );
    if (lastSep < 2) return null;
    // If last separator is index 2 (right after C:), parent is the drive root WITH trailing slash (C:\)
    if (lastSep === 2) {
      return units.slice(0, 3);
    }
    return units.slice(0, lastSep);
  }

  // 3. Extended UNC: \\?\UNC\server\share\...
  if (isExtendedUnc) {
    const serverSlash = units.slice(8, end).indexOf(92);
    if (serverSlash < 0) return null;
    const shareStart = 8 + serverSlash + 1;
    const shareSlash = units.slice(shareStart, end).indexOf(92);
    if (shareSlash < 0) return null; // At share root: \\?\UNC\server\share
    const shareEnd = shareStart + shareSlash;
    const lastSep = Math.max(
      units.slice(0, end).lastIndexOf(92),
      units.slice(0, end).lastIndexOf(47)
    );
    if (lastSep <= shareEnd) {
      return units.slice(0, shareEnd);
    }
    return units.slice(0, lastSep);
  }

  // 4. Standard UNC: \\server\share\...
  if (end >= 2 && units[0] === 92 && units[1] === 92) {
    const serverSlash = units.slice(2, end).indexOf(92);
    if (serverSlash < 0) return null;
    const shareStart = 2 + serverSlash + 1;
    const shareSlash = units.slice(shareStart, end).indexOf(92);
    if (shareSlash < 0) return null; // At share root: \\server\share
    const shareEnd = shareStart + shareSlash;
    const lastSep = Math.max(
      units.slice(0, end).lastIndexOf(92),
      units.slice(0, end).lastIndexOf(47)
    );
    if (lastSep <= shareEnd) {
      return units.slice(0, shareEnd);
    }
    return units.slice(0, lastSep);
  }

  // 5. Generic or relative path
  const lastSep = Math.max(
    units.slice(0, end).lastIndexOf(92),
    units.slice(0, end).lastIndexOf(47)
  );
  if (lastSep <= 0) return null;
  return units.slice(0, lastSep);
}

export function getParentPathString(path: string): string | null {
  if (!path) return null;
  const codes = Array.from(path).map((c) => c.charCodeAt(0));
  const parentCodes = getParentPathUtf16(codes);
  if (!parentCodes) return null;
  return String.fromCharCode(...parentCodes);
}

export function getParentPath(
  nativeUnits?: number[],
  displayPath?: string
): { utf16?: number[]; display?: string } | null {
  if (nativeUnits && nativeUnits.length > 0) {
    const parentUnits = getParentPathUtf16(nativeUnits);
    if (parentUnits) {
      return { utf16: parentUnits };
    }
  }
  if (displayPath && displayPath.length > 0) {
    const parentStr = getParentPathString(displayPath);
    if (parentStr) {
      return { display: parentStr };
    }
  }
  return null;
}

export function sameWindowsPath(left: number[], right: number[]): boolean {
  if (left.length !== right.length) return false;
  for (let index = 0; index < left.length; index += 1) {
    const normalize = (unit: number) => (unit >= 65 && unit <= 90 ? unit + 32 : unit);
    if (normalize(left[index]) !== normalize(right[index])) return false;
  }
  return true;
}
