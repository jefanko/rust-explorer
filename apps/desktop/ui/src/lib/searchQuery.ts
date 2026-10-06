export const MIN_INDEXED_QUERY_LENGTH = 3;

const METADATA_FILTER = /(?:ext:|\*\.|\.)[a-z0-9_-]+|type:(?:folder|file)/i;
const METADATA_FILTER_GLOBAL = /(?:ext:|\*\.|\.)[a-z0-9_-]+|type:[^\s]+/gi;

export interface IndexedQueryValidation {
  valid: boolean;
  error?: string;
}

/**
 * Indexed (trigram) search needs at least three characters of free text,
 * unless the query carries a metadata filter such as `ext:pdf`, `*.exe` or `type:folder`.
 */
export function validateIndexedQuery(query: string): IndexedQueryValidation {
  const trimmed = query.trim();
  if (!trimmed) return { valid: false };
  const hasMetadataFilter = METADATA_FILTER.test(trimmed);
  const textWithoutFilters = trimmed.replace(METADATA_FILTER_GLOBAL, "").trim();
  if (!hasMetadataFilter && textWithoutFilters.length < MIN_INDEXED_QUERY_LENGTH) {
    return { valid: false, error: "Use at least 3 characters for indexed search" };
  }
  return { valid: true };
}
