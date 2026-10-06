import React from "react";
import type { TabState } from "../state/tabs";
import { getBreadcrumbs } from "../lib/paths";
import { FluentIcon } from "./FluentIcon";

interface NavToolbarProps {
  activeTab: TabState;
  canGoUp: boolean;
  onGoBack: () => void;
  onGoForward: () => void;
  onGoUp: () => void;
  onNavigate: (path: string) => void;
  onRefresh: () => void;
  isEditingAddress: boolean;
  setIsEditingAddress: (editing: boolean) => void;
  addressInputRef: React.RefObject<HTMLInputElement>;
  filterInputRef: React.RefObject<HTMLInputElement>;
  onAddressInputChange: (val: string) => void;
  searchScope: "folder" | "indexed";
  onSetSearchScope: (scope: "folder" | "indexed") => void;
  indexedSearchQuery: string;
  onIndexedSearchQueryChange: (val: string) => void;
  onFilterQueryChange: (val: string) => void;
  onSearchSubfolders: (override?: string) => void;
  searchLoading: boolean;
}

export const NavToolbar: React.FC<NavToolbarProps> = ({
  activeTab,
  canGoUp,
  onGoBack,
  onGoForward,
  onGoUp,
  onNavigate,
  onRefresh,
  isEditingAddress,
  setIsEditingAddress,
  addressInputRef,
  filterInputRef,
  onAddressInputChange,
  searchScope,
  onSetSearchScope,
  indexedSearchQuery,
  onIndexedSearchQueryChange,
  onFilterQueryChange,
  onSearchSubfolders,
  searchLoading,
}) => {
  return (
    <div className="nav-toolbar">
      <div className="nav-buttons">
        <button
          className="nav-btn"
          onClick={onGoBack}
          disabled={activeTab.historyIndex <= 0}
          aria-label="Back"
          title="Back (Alt+Left)"
        >
          <FluentIcon name="arrow-left" size={14} />
        </button>
        <button
          className="nav-btn"
          onClick={onGoForward}
          disabled={activeTab.historyIndex >= activeTab.history.length - 1}
          aria-label="Forward"
          title="Forward (Alt+Right)"
        >
          <FluentIcon name="arrow-right" size={14} />
        </button>
        <button
          className="nav-btn"
          onClick={onGoUp}
          disabled={!canGoUp}
          aria-label="Up"
          title={canGoUp ? "Up to Parent (Alt+Up)" : "At root (Alt+Up)"}
        >
          <FluentIcon name="arrow-up" size={14} />
        </button>
      </div>

      <div className="address-bar-wrapper">
        <span className="address-folder-icon" aria-hidden="true">
          <FluentIcon name="folder" size={14} />
        </span>
        {!isEditingAddress ? (
          <div
            className="address-breadcrumbs-container"
            onClick={() => {
              setIsEditingAddress(true);
              setTimeout(() => {
                addressInputRef.current?.focus();
                addressInputRef.current?.select();
              }, 50);
            }}
            title="Click to edit path (Ctrl+L / Alt+D)"
          >
            <div className="breadcrumb-segments">
              {getBreadcrumbs(activeTab.path).map((crumb, idx, arr) => (
                <span key={crumb.fullPath} className="breadcrumb-segment-wrapper">
                  <button
                    type="button"
                    className="breadcrumb-segment-btn"
                    onClick={(e) => {
                      e.stopPropagation();
                      onNavigate(crumb.fullPath);
                    }}
                    title={crumb.fullPath}
                  >
                    {crumb.label}
                  </button>
                  {idx < arr.length - 1 && (
                    <FluentIcon name="chevron-right" size={10} className="breadcrumb-chevron" />
                  )}
                </span>
              ))}
            </div>
            <button
              type="button"
              className="address-refresh-btn"
              onClick={(e) => {
                e.stopPropagation();
                onRefresh();
              }}
              title="Refresh (F5)"
            >
              <FluentIcon name="refresh" size={13} />
            </button>
          </div>
        ) : (
          <form
            className="address-edit-form"
            onSubmit={(e) => {
              e.preventDefault();
              setIsEditingAddress(false);
              onNavigate(activeTab.addressInput);
            }}
          >
            <input
              ref={addressInputRef}
              type="text"
              className="address-input"
              value={activeTab.addressInput}
              onChange={(e) => onAddressInputChange(e.target.value)}
              onBlur={() => setIsEditingAddress(false)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  onAddressInputChange(activeTab.path);
                  setIsEditingAddress(false);
                }
              }}
              aria-label="Address"
              placeholder="Enter a path... (Ctrl+L)"
            />
            <button
              type="button"
              className="address-refresh-btn"
              onClick={onRefresh}
              title="Refresh (F5)"
            >
              <FluentIcon name="refresh" size={13} />
            </button>
          </form>
        )}
      </div>

      <div className="search-container">
        <div className="search-input-wrapper">
          <span className="search-icon" aria-hidden="true">
            <FluentIcon name="search" size={13} />
          </span>
          <input
            ref={filterInputRef}
            type="text"
            className="search-input"
            placeholder={
              searchScope === "folder"
                ? "Filter current folder..."
                : "Search subfolders (e.g. *.exe)..."
            }
            value={searchScope === "folder" ? activeTab.filterQuery : indexedSearchQuery}
            onChange={(e) => {
              if (searchScope === "folder") {
                onFilterQueryChange(e.target.value);
              } else {
                onIndexedSearchQueryChange(e.target.value);
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && searchScope === "folder" && activeTab.filterQuery.trim()) {
                e.preventDefault();
                onSearchSubfolders();
              }
            }}
            aria-label={searchScope === "folder" ? "Filter current folder" : "Search subfolders"}
          />
          {searchScope === "folder" && activeTab.filterQuery.trim().length >= 2 && (
            <button
              type="button"
              className="search-subfolders-quick-btn"
              onClick={() => onSearchSubfolders()}
              title="Search inside subfolders (Indexed)"
            >
              <FluentIcon name="search" size={11} />
              <span>Subfolders</span>
            </button>
          )}
          {searchScope === "folder" ? (
            <button
              type="button"
              className="search-scope-pill"
              onClick={() => {
                onSetSearchScope("indexed");
                if (activeTab.filterQuery && !indexedSearchQuery) {
                  onIndexedSearchQueryChange(activeTab.filterQuery);
                }
                filterInputRef.current?.focus();
              }}
              title="Switch to searching all subfolders"
            >
              Current
            </button>
          ) : (
            <button
              type="button"
              className="search-scope-pill active"
              onClick={() => {
                onSetSearchScope("folder");
                filterInputRef.current?.focus();
              }}
              title="Switch to filtering current folder"
            >
              Subfolders
            </button>
          )}
          {searchScope === "indexed" && searchLoading && (
            <span className="search-spinner" title="Searching...">
              <FluentIcon name="refresh" size={12} />
            </span>
          )}
        </div>
      </div>
    </div>
  );
};
