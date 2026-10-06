import React from "react";
import type { JobSummary } from "../bridge/types";
import type { ThemeMode } from "../app/types";
import { FluentIcon } from "./FluentIcon";

interface CommandBarProps {
  selectedCount: number;
  showHiddenFiles: boolean;
  showPreviewPane: boolean;
  showJobsDrawer: boolean;
  theme: ThemeMode;
  jobs: JobSummary[];
  onCreateFolder: () => void;
  onCut: () => void;
  onCopy: () => void;
  onPaste: () => void;
  onRename: () => void;
  onRecycle: () => void;
  onToggleHiddenFiles: () => void;
  onTogglePreviewPane: () => void;
  onToggleJobsDrawer: () => void;
  onToggleTheme: () => void;
}

export const CommandBar: React.FC<CommandBarProps> = ({
  selectedCount,
  showHiddenFiles,
  showPreviewPane,
  showJobsDrawer,
  theme,
  jobs,
  onCreateFolder,
  onCut,
  onCopy,
  onPaste,
  onRename,
  onRecycle,
  onToggleHiddenFiles,
  onTogglePreviewPane,
  onToggleJobsDrawer,
  onToggleTheme,
}) => {
  const activeJobsCount = jobs.filter(
    (j) => j.state === "running" || j.state === "validating"
  ).length;

  return (
    <div className="command-bar">
      <button
        type="button"
        className="command-btn primary-action"
        onClick={onCreateFolder}
        title="New Folder (Ctrl+Shift+N)"
        aria-label="New folder"
      >
        <span className="command-icon">
          <FluentIcon name="folder-add" size={16} />
        </span>
        <span>New</span>
      </button>

      <span className="command-divider" aria-hidden="true" />

      <button
        type="button"
        className="command-btn"
        onClick={onCut}
        disabled={selectedCount === 0}
        title="Cut (Ctrl+X)"
        aria-label="Cut"
      >
        <span className="command-icon">
          <FluentIcon name="cut" size={16} />
        </span>
        <span>Cut</span>
      </button>

      <button
        type="button"
        className="command-btn"
        onClick={onCopy}
        disabled={selectedCount === 0}
        title="Copy (Ctrl+C)"
        aria-label="Copy"
      >
        <span className="command-icon">
          <FluentIcon name="copy" size={16} />
        </span>
        <span>Copy</span>
      </button>

      <button
        type="button"
        className="command-btn"
        onClick={onPaste}
        title="Paste (Ctrl+V)"
        aria-label="Paste"
      >
        <span className="command-icon">
          <FluentIcon name="paste" size={16} />
        </span>
        <span>Paste</span>
      </button>

      <button
        type="button"
        className="command-btn"
        onClick={onRename}
        disabled={selectedCount !== 1}
        title="Rename (F2)"
        aria-label="Rename"
      >
        <span className="command-icon">
          <FluentIcon name="rename" size={16} />
        </span>
        <span>Rename</span>
      </button>

      <button
        type="button"
        className="command-btn"
        onClick={onRecycle}
        disabled={selectedCount === 0}
        title="Recycle (Delete)"
        aria-label="Delete"
      >
        <span className="command-icon">
          <FluentIcon name="delete" size={16} />
        </span>
        <span>Delete</span>
      </button>

      <span className="command-divider" aria-hidden="true" />

      <button
        type="button"
        className={`command-btn ${showHiddenFiles ? "active" : ""}`}
        onClick={onToggleHiddenFiles}
        title="Show or hide hidden files"
        aria-label="Toggle hidden files"
        aria-pressed={showHiddenFiles}
      >
        <span className="command-icon">
          <FluentIcon name={showHiddenFiles ? "eye" : "eye-off"} size={16} />
        </span>
        <span>{showHiddenFiles ? "Hidden: On" : "Hidden: Off"}</span>
      </button>

      <button
        type="button"
        className={`command-btn ${showPreviewPane ? "active" : ""}`}
        onClick={onTogglePreviewPane}
        title="Toggle File Preview Pane (Ctrl+P / Alt+P)"
        aria-label="Toggle preview pane"
        aria-pressed={showPreviewPane}
      >
        <span className="command-icon">
          <FluentIcon name="eye" size={16} />
        </span>
        <span>Preview</span>
      </button>

      <span className="command-spacer" />

      <button
        type="button"
        className={`command-btn ${showJobsDrawer ? "active" : ""}`}
        onClick={onToggleJobsDrawer}
        title="Toggle Background Jobs Activity"
      >
        <span className="command-icon">
          <FluentIcon name="activity" size={16} />
        </span>
        <span>Activity</span>
        {activeJobsCount > 0 && (
          <span className="jobs-count-badge">{activeJobsCount}</span>
        )}
      </button>

      <button
        type="button"
        className="command-btn"
        onClick={onToggleTheme}
        title={`Current theme: ${theme}. Click to switch theme.`}
        aria-label="Toggle theme"
      >
        <span className="command-icon">
          <FluentIcon
            name={
              theme === "system"
                ? "theme-system"
                : theme === "dark"
                ? "theme-moon"
                : "theme-sun"
            }
            size={16}
          />
        </span>
      </button>
    </div>
  );
};
