import React from "react";
import type { ContextMenuState } from "../app/types";
import type { FileEntry, SearchResultItem } from "../bridge/types";
import { FluentIcon } from "./FluentIcon";

interface ContextMenuProps {
  contextMenu: ContextMenuState;
  activePath: string;
  onClose: () => void;
  onOpenEntry: (entry: FileEntry) => void;
  onOpenSearchResult: (item: SearchResultItem) => void;
  onOpenContainingFolder: (item: SearchResultItem) => void;
  onCopySearchResultPath: (item: SearchResultItem) => void;
  onCut: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onRefresh: () => void;
  onOpenRenameModal: (entry?: FileEntry) => void;
  onOpenRecycleModal: (entry?: FileEntry) => void;
  onOpenCreateFolderModal: () => void;
  onOpenInExplorer: (entryToken: string | null) => void;
  onAddFavorite: (path: string) => void;
  onShowProperties: (entryToken: string | null) => void;
}

export const ContextMenu: React.FC<ContextMenuProps> = ({
  contextMenu,
  activePath,
  onClose,
  onOpenEntry,
  onOpenSearchResult,
  onOpenContainingFolder,
  onCopySearchResultPath,
  onCut,
  onCopy,
  onPaste,
  onRefresh,
  onOpenRenameModal,
  onOpenRecycleModal,
  onOpenCreateFolderModal,
  onOpenInExplorer,
  onAddFavorite,
  onShowProperties,
}) => {
  return (
    <div
      className="context-menu"
      style={{ top: `${contextMenu.y}px`, left: `${contextMenu.x}px` }}
      onClick={(e) => e.stopPropagation()}
    >
      {contextMenu.searchResult ? (
        <>
          <div
            className="context-menu-item"
            onClick={() => {
              const item = contextMenu.searchResult!;
              onClose();
              onOpenSearchResult(item);
            }}
          >
            <span>Open</span>
            <span className="context-menu-shortcut">Enter</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              const item = contextMenu.searchResult!;
              onClose();
              onOpenContainingFolder(item);
            }}
          >
            <span>Open containing folder</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              const item = contextMenu.searchResult!;
              onClose();
              onCopySearchResultPath(item);
            }}
          >
            <span>Copy path</span>
          </div>
        </>
      ) : contextMenu.entry ? (
        <>
          <div className="context-menu-quick-actions">
            <button
              type="button"
              className="context-quick-btn"
              title="Cut (Ctrl+X)"
              onClick={() => {
                onClose();
                onCut();
              }}
            >
              <FluentIcon name="cut" size={15} />
            </button>
            <button
              type="button"
              className="context-quick-btn"
              title="Copy (Ctrl+C)"
              onClick={() => {
                onClose();
                onCopy();
              }}
            >
              <FluentIcon name="copy" size={15} />
            </button>
            <button
              type="button"
              className="context-quick-btn"
              title="Rename (F2)"
              onClick={() => {
                const entry = contextMenu.entry!;
                onClose();
                onOpenRenameModal(entry);
              }}
            >
              <FluentIcon name="rename" size={15} />
            </button>
            <button
              type="button"
              className="context-quick-btn"
              title="Delete (Del)"
              onClick={() => {
                const entry = contextMenu.entry!;
                onClose();
                onOpenRecycleModal(entry);
              }}
            >
              <FluentIcon name="delete" size={15} />
            </button>
          </div>

          <div
            className="context-menu-item"
            onClick={() => {
              const entry = contextMenu.entry!;
              onClose();
              onOpenEntry(entry);
            }}
          >
            <span>Open</span>
            <span className="context-menu-shortcut">Enter</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onCopy();
            }}
          >
            <span>Copy</span>
            <span className="context-menu-shortcut">Ctrl+C</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onCut();
            }}
          >
            <span>Cut</span>
            <span className="context-menu-shortcut">Ctrl+X</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              const entry = contextMenu.entry!;
              onClose();
              onOpenRenameModal(entry);
            }}
          >
            <span>Rename</span>
            <span className="context-menu-shortcut">F2</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              const entry = contextMenu.entry!;
              onClose();
              onOpenRecycleModal(entry);
            }}
          >
            <span>Delete</span>
            <span className="context-menu-shortcut">Del</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              const entry = contextMenu.entry!;
              onClose();
              onOpenInExplorer(entry.token);
            }}
          >
            <span>Open in Windows Explorer</span>
          </div>
          {contextMenu.entry.kind === "directory" && (
            <div
              className="context-menu-item"
              onClick={() => {
                const entry = contextMenu.entry!;
                onClose();
                const fullPath = activePath.endsWith("\\")
                  ? activePath + entry.display_name
                  : activePath + "\\" + entry.display_name;
                onAddFavorite(fullPath);
              }}
            >
              <span>Add to Favorites</span>
            </div>
          )}
          <div className="context-menu-separator" />
          <div
            className="context-menu-item"
            onClick={() => {
              const entry = contextMenu.entry!;
              onClose();
              onShowProperties(entry.token);
            }}
          >
            <span>Properties</span>
            <span className="context-menu-shortcut">Alt+Enter</span>
          </div>
        </>
      ) : (
        <>
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onOpenCreateFolderModal();
            }}
          >
            <span>New Folder</span>
            <span className="context-menu-shortcut">Ctrl+Shift+N</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onPaste();
            }}
          >
            <span>Paste</span>
            <span className="context-menu-shortcut">Ctrl+V</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onRefresh();
            }}
          >
            <span>Refresh</span>
            <span className="context-menu-shortcut">F5</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onOpenInExplorer(null);
            }}
          >
            <span>Open in Windows Explorer</span>
          </div>
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onAddFavorite(activePath);
            }}
          >
            <span>Add Current Folder to Favorites</span>
          </div>
          <div className="context-menu-separator" />
          <div
            className="context-menu-item"
            onClick={() => {
              onClose();
              onShowProperties(null);
            }}
          >
            <span>Properties</span>
            <span className="context-menu-shortcut">Alt+Enter</span>
          </div>
        </>
      )}
    </div>
  );
};
