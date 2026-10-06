/** Title and summary value for the recycle confirmation dialog. */
export function recycleDialogText(displayNames: string[]): { title: string; value: string } {
  if (displayNames.length === 1) {
    return { title: `Recycle "${displayNames[0]}"?`, value: displayNames[0] };
  }
  return {
    title: `Recycle ${displayNames.length} items?`,
    value: `${displayNames.length} items`,
  };
}

/** Label for the bold part of the confirmation sentence. */
export function recycleTargetLabel(value: string, count: number): string {
  return count === 1 ? `"${value}"` : `${count} items`;
}
