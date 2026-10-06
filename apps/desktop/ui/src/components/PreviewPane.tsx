import React from "react";
import type { FileEntry } from "../bridge/types";
import { formatBytes, formatFiletime } from "../lib/format";
import { FluentIcon } from "./FluentIcon";
import { getFileIcon } from "./icons";

export type PreviewData =
  | {
      kind: "text";
      content: string;
      truncated: boolean;
      encoding: string;
      line_count: number;
    }
  | {
      kind: "image";
      mime: string;
      data_base64: string;
      byte_len: number;
    }
  | {
      kind: "folder";
      item_count?: number | null;
    }
  | {
      kind: "unsupported";
      reason: string;
    };

interface PreviewPaneProps {
  entry: FileEntry | null;
  preview: PreviewData | null;
  loading: boolean;
  error: string | null;
  onClose: () => void;
}

export const PreviewPane: React.FC<PreviewPaneProps> = ({
  entry,
  preview,
  loading,
  error,
  onClose,
}) => {
  if (!entry) {
    return (
      <aside className="preview-pane empty">
        <div className="preview-header">
          <span>Preview</span>
          <button className="preview-close-btn" onClick={onClose} title="Close Preview">
            <FluentIcon name="close" size={10} />
          </button>
        </div>
        <div className="preview-empty-note">Select a file to preview</div>
      </aside>
    );
  }

  return (
    <aside className="preview-pane">
      <div className="preview-header">
        <span className="preview-title">Preview</span>
        <button
          className="preview-close-btn"
          onClick={onClose}
          title="Close Preview (Ctrl+P / Alt+P)"
        >
          <FluentIcon name="close" size={10} />
        </button>
      </div>

      <div className="preview-body">
        {/* Meta summary card */}
        <div className="preview-meta">
          <div className="preview-icon">{getFileIcon(entry, 32)}</div>
          <div className="preview-meta-details">
            <div className="preview-filename" title={entry.display_name}>
              {entry.display_name}
            </div>
            <div className="preview-subtext">
              {entry.kind === "directory"
                ? "File folder"
                : entry.extension
                ? `${entry.extension.toUpperCase()} File`
                : "File"}
            </div>
            {entry.size_bytes !== null && entry.size_bytes !== undefined && (
              <div className="preview-subtext">Size: {formatBytes(entry.size_bytes)}</div>
            )}
            {entry.modified_filetime && (
              <div className="preview-subtext">
                Modified: {formatFiletime(entry.modified_filetime)}
              </div>
            )}
          </div>
        </div>

        {/* Content Preview */}
        <div className="preview-content">
          {loading ? (
            <div className="preview-loading">
              <FluentIcon name="refresh" size={16} className="search-spinner" />
              <span>Loading preview...</span>
            </div>
          ) : error ? (
            <div className="preview-error">
              <FluentIcon name="warning" size={16} />
              <span>{error}</span>
            </div>
          ) : !preview ? (
            <div className="preview-empty-note">No preview available</div>
          ) : preview.kind === "text" ? (
            <div className="preview-text-container">
              <div className="preview-text-info">
                <span>
                  {preview.encoding.toUpperCase()} · {preview.line_count} lines
                  {preview.truncated && " (First 64 KiB shown)"}
                </span>
              </div>
              <pre className="preview-text-content">{preview.content}</pre>
            </div>
          ) : preview.kind === "image" ? (
            <div className="preview-image-container">
              <img
                src={`data:${preview.mime};base64,${preview.data_base64}`}
                alt={entry.display_name}
                className="preview-image"
              />
            </div>
          ) : preview.kind === "folder" ? (
            <div className="preview-folder-info">
              <FluentIcon name="folder" size={32} />
              <p>
                {preview.item_count !== null && preview.item_count !== undefined
                  ? `${preview.item_count} items in folder`
                  : "Folder"}
              </p>
            </div>
          ) : (
            <div className="preview-unsupported">
              <p>{preview.reason}</p>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
};
