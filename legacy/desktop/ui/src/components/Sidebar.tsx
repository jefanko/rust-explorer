import React, { useState } from "react";
import type { DriveItem, KnownFolderItem, IndexedRoot } from "../bridge/types";
import { formatBytes } from "../lib/format";
import { getTabTitle } from "../lib/paths";
import { FluentIcon } from "./FluentIcon";
import { getKnownFolderIcon } from "./icons";

interface SidebarProps {
  favorites: string[];
  knownFolders: KnownFolderItem[];
  drives: DriveItem[];
  indexedRoots: IndexedRoot[];
  activePath: string;
  hasActiveFolderToken: boolean;
  onNavigate: (path: string | number[]) => void;
  onRemoveFavorite: (path: string) => void;
  onIndexCurrentFolder: () => void;
  onRecrawlRoot: (id: string) => void;
  onRemoveRoot: (id: string) => void;
  onDropOnPath?: (destinationPath: string) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  favorites,
  knownFolders,
  drives,
  indexedRoots,
  activePath,
  hasActiveFolderToken,
  onNavigate,
  onRemoveFavorite,
  onIndexCurrentFolder,
  onRecrawlRoot,
  onRemoveRoot,
  onDropOnPath,
}) => {
  const [dragOverPath, setDragOverPath] = useState<string | null>(null);

  const handleDragOver = (path: string, e: React.DragEvent) => {
    if (!onDropOnPath) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = e.ctrlKey ? "copy" : "move";
    if (dragOverPath !== path) setDragOverPath(path);
  };

  const handleDragLeave = (path: string) => {
    if (dragOverPath === path) setDragOverPath(null);
  };

  const handleDrop = (path: string, e: React.DragEvent) => {
    if (!onDropOnPath) return;
    e.preventDefault();
    setDragOverPath(null);
    onDropOnPath(path);
  };

  return (
    <aside className="sidebar">
      {/* Favorites */}
      <section className="sidebar-section">
        <h3>Favorites</h3>
        {favorites.length === 0 ? (
          <div className="sidebar-empty-note">No pinned favorites</div>
        ) : (
          <ul>
            {favorites.map((favPath) => (
              <li
                key={favPath}
                className={`${activePath === favPath ? "active" : ""} ${dragOverPath === favPath ? "drop-target" : ""}`}
                onClick={() => onNavigate(favPath)}
                onDragOver={(e) => handleDragOver(favPath, e)}
                onDragLeave={() => handleDragLeave(favPath)}
                onDrop={(e) => handleDrop(favPath, e)}
              >
                <div className="sidebar-fav-item">
                  <span className="sidebar-icon">
                    <FluentIcon name="star" size={16} />
                  </span>
                  <span className="sidebar-label" title={favPath}>
                    {getTabTitle(favPath)}
                  </span>
                </div>
                <button
                  className="fav-remove-btn"
                  title="Remove from favorites"
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemoveFavorite(favPath);
                  }}
                >
                  <FluentIcon name="close" size={10} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* Known Folders */}
      <section className="sidebar-section">
        <h3>Quick Access</h3>
        <ul>
          {knownFolders.map((kf) => (
            <li
              key={kf.id}
              onClick={() => onNavigate(kf.path)}
              className={`${activePath === kf.path ? "active" : ""} ${dragOverPath === kf.path ? "drop-target" : ""}`}
              onDragOver={(e) => handleDragOver(kf.path, e)}
              onDragLeave={() => handleDragLeave(kf.path)}
              onDrop={(e) => handleDrop(kf.path, e)}
            >
              <div className="sidebar-fav-item">
                <span className="sidebar-icon">{getKnownFolderIcon(kf.name)}</span>
                <span className="sidebar-label">{kf.name}</span>
              </div>
            </li>
          ))}
        </ul>
      </section>

      {/* Drives */}
      <section className="sidebar-section">
        <h3>This PC</h3>
        <ul>
          {drives.map((d) => {
            const hasMetrics = d.total_bytes && d.total_bytes > 0;
            const usedBytes = hasMetrics ? d.total_bytes! - (d.free_bytes || 0) : 0;
            const usedPct = hasMetrics ? Math.round((usedBytes / d.total_bytes!) * 100) : 0;
            return (
              <li
                key={d.path}
                onClick={() => onNavigate(d.path)}
                className={`${activePath === d.path ? "active" : ""} ${dragOverPath === d.path ? "drop-target" : ""}`}
                title={
                  hasMetrics
                    ? `${formatBytes(d.free_bytes)} free of ${formatBytes(d.total_bytes)}`
                    : d.path
                }
                onDragOver={(e) => handleDragOver(d.path, e)}
                onDragLeave={() => handleDragLeave(d.path)}
                onDrop={(e) => handleDrop(d.path, e)}
              >
                <div className="sidebar-fav-item">
                  <span className="sidebar-icon">
                    <FluentIcon name="drive" size={16} />
                  </span>
                  <div className="drive-info-container">
                    <span className="sidebar-label">{d.name}</span>
                    {hasMetrics && (
                      <>
                        <div className="drive-capacity-bar">
                          <div
                            className={`drive-capacity-fill ${usedPct > 90 ? "high-usage" : ""}`}
                            style={{ width: `${usedPct}%` }}
                          />
                        </div>
                        <span className="drive-capacity-text">
                          {formatBytes(d.free_bytes)} free
                        </span>
                      </>
                    )}
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      </section>

      {/* Indexed Roots */}
      <section className="sidebar-section">
        <div className="sidebar-section-header">
          <h3>Indexed Roots ({indexedRoots.length}/8)</h3>
          <button
            className="sidebar-add-btn"
            title="Index current folder"
            disabled={indexedRoots.length >= 8 || !hasActiveFolderToken}
            onClick={onIndexCurrentFolder}
          >
            <FluentIcon name="add" size={12} />
          </button>
        </div>
        {indexedRoots.length === 0 ? (
          <div className="sidebar-empty-note">
            No indexed roots
            {hasActiveFolderToken && (
              <button className="btn-link" onClick={onIndexCurrentFolder}>
                Index current folder
              </button>
            )}
          </div>
        ) : (
          <ul className="indexed-roots-list">
            {indexedRoots.map((root) => (
              <li key={root.id} className="indexed-root-item">
                <div
                  className="indexed-root-info"
                  onClick={() => onNavigate(root.path_utf16)}
                  title={`Path: ${root.display_path}\nState: ${root.state}\nEpoch: ${root.completed_epoch}`}
                >
                  <span className="root-name">{getTabTitle(root.display_path)}</span>
                  <span className={`root-badge badge-${root.state}`}>{root.state}</span>
                </div>
                <div className="root-actions">
                  <button
                    className="root-recrawl-btn"
                    title={root.state === "scanning" ? "Scanning index..." : "Re-crawl index"}
                    disabled={root.state === "scanning"}
                    onClick={(e) => {
                      e.stopPropagation();
                      onRecrawlRoot(root.id);
                    }}
                  >
                    <FluentIcon
                      name="refresh"
                      size={12}
                      className={root.state === "scanning" ? "spinning" : ""}
                    />
                  </button>
                  <button
                    className="fav-remove-btn"
                    title="Remove from index"
                    onClick={(e) => {
                      e.stopPropagation();
                      onRemoveRoot(root.id);
                    }}
                  >
                    <FluentIcon name="close" size={10} />
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </aside>
  );
};
