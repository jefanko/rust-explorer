import React from "react";
import type { JobSummary } from "../bridge/types";
import { FluentIcon } from "./FluentIcon";

interface StatusBarProps {
  isSearchActive: boolean;
  searchTotalMatches: number;
  searchIsCapped: boolean;
  selectedSearchIndex: number;
  displayedEntriesCount: number;
  visibleEntryCount: number;
  filterQuery: string;
  selectedCount: number;
  activePath: string;
  jobs: JobSummary[];
  showJobsDrawer: boolean;
  onToggleJobsDrawer: () => void;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  isSearchActive,
  searchTotalMatches,
  searchIsCapped,
  selectedSearchIndex,
  displayedEntriesCount,
  visibleEntryCount,
  filterQuery,
  selectedCount,
  activePath,
  jobs,
  showJobsDrawer,
  onToggleJobsDrawer,
}) => {
  return (
    <footer className="app-statusbar">
      <span>
        {isSearchActive ? (
          <>
            {searchTotalMatches} {searchTotalMatches === 1 ? "match" : "matches"}
            {searchIsCapped && " (First 1,000 matches; refine search)"}
            {selectedSearchIndex >= 0 && " | 1 selected"}
          </>
        ) : (
          <>
            {displayedEntriesCount} {displayedEntriesCount === 1 ? "item" : "items"}
            {filterQuery && ` (filtered from ${visibleEntryCount})`}
            {selectedCount > 0 && ` | ${selectedCount} selected`}
          </>
        )}
      </span>
      <span className="status-spacer" />
      <button
        className={`action-btn ${showJobsDrawer ? "active" : ""}`}
        style={{
          padding: "2px 8px",
          fontSize: "11px",
          marginRight: "8px",
          display: "inline-flex",
          alignItems: "center",
          gap: "4px",
        }}
        onClick={onToggleJobsDrawer}
        title="Toggle Jobs Drawer"
      >
        <FluentIcon name="activity" size={12} />
        <span>Jobs {jobs.length > 0 && `(${jobs.length})`}</span>
      </button>
      <span>{activePath}</span>
    </footer>
  );
};
