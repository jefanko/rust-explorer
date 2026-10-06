/** Tokens of every entry between two indices (inclusive, order-independent, clamped to the list). */
export function tokensInRange(
  entries: ReadonlyArray<{ token: string }>,
  anchor: number,
  target: number
): Set<string> {
  const start = Math.max(0, Math.min(anchor, target));
  const end = Math.min(entries.length - 1, Math.max(anchor, target));
  const tokens = new Set<string>();
  for (let i = start; i <= end; i++) {
    tokens.add(entries[i].token);
  }
  return tokens;
}

export interface SelectionMove {
  focusedIndex: number;
  anchorIndex: number;
  selectedTokens: Set<string>;
}

/**
 * Computes the selection after moving focus to `target`.
 * With `extend` (Shift) the range from the anchor to the target is selected,
 * otherwise only the target row is selected and becomes the new anchor.
 */
export function moveSelection(
  entries: ReadonlyArray<{ token: string }>,
  currentAnchor: number,
  target: number,
  extend: boolean
): SelectionMove | null {
  if (entries.length === 0) return null;
  const clamped = Math.max(0, Math.min(entries.length - 1, target));
  if (extend) {
    const anchor = currentAnchor >= 0 ? currentAnchor : clamped;
    return {
      focusedIndex: clamped,
      anchorIndex: anchor,
      selectedTokens: tokensInRange(entries, anchor, clamped),
    };
  }
  return {
    focusedIndex: clamped,
    anchorIndex: clamped,
    selectedTokens: new Set([entries[clamped].token]),
  };
}

/** Resolves the next focus index for an Arrow key press (first/last row when nothing is focused). */
export function nextFocusIndex(focusedIndex: number, delta: number, length: number): number {
  if (length === 0) return -1;
  const next = focusedIndex === -1 ? (delta > 0 ? 0 : length - 1) : focusedIndex + delta;
  return Math.max(0, Math.min(length - 1, next));
}
