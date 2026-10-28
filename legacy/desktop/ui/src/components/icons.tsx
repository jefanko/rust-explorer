import { FluentIcon, getFluentFileIcon, getFluentKnownFolderIcon } from "./FluentIcon";

export function getFileIcon(entry: { kind: string; extension: string }, size = 16) {
  return <FluentIcon name={getFluentFileIcon(entry)} size={size} />;
}

export function getKnownFolderIcon(name: string) {
  return <FluentIcon name={getFluentKnownFolderIcon(name)} size={16} />;
}
