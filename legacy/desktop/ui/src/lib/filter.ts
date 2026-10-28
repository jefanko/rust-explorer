export function matchFilterQuery(
  entry: { display_name: string; extension: string; kind: string },
  filterQuery: string
): boolean {
  const q = filterQuery.trim().toLowerCase();
  if (!q) return true;

  // Direct extension filter like *.exe or *.pdf
  if (q.startsWith("*.")) {
    const ext = q.slice(2).trim();
    if (ext && ext !== "*") {
      return (
        entry.extension.toLowerCase() === ext ||
        entry.display_name.toLowerCase().endsWith("." + ext)
      );
    }
  }

  // Direct dot-extension filter like .exe or .pdf
  if (
    q.startsWith(".") &&
    q.length > 1 &&
    !q.slice(1).includes(".") &&
    !q.includes(" ") &&
    !q.includes(":") &&
    !q.includes("/") &&
    !q.includes("\\")
  ) {
    const ext = q.slice(1);
    return (
      entry.extension.toLowerCase() === ext ||
      entry.display_name.toLowerCase().endsWith("." + ext) ||
      entry.display_name.toLowerCase().includes(q)
    );
  }

  // ext: filter
  if (q.startsWith("ext:")) {
    const ext = q.slice(4).trim().replace(/^\./, "");
    return entry.extension.toLowerCase() === ext;
  }

  // type: filter
  if (q.startsWith("type:")) {
    const type = q.slice(5).trim();
    if (type === "folder" || type === "dir" || type === "directory") {
      return entry.kind === "directory";
    }
    if (type === "file") {
      return entry.kind === "file";
    }
  }

  // Wildcard glob if contains * or ?
  if (q.includes("*") || q.includes("?")) {
    const regexPattern =
      "^" +
      q
        .replace(/[-/\\^$+.,{}[\]()]/g, "\\$&")
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".") +
      "$";
    try {
      const re = new RegExp(regexPattern, "i");
      return re.test(entry.display_name);
    } catch {
      // Fall back to substring match below
    }
  }

  return entry.display_name.toLowerCase().includes(q);
}
