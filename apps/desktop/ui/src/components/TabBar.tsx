import React from "react";
import type { TabState } from "../state/tabs";
import { FluentIcon } from "./FluentIcon";

interface TabBarProps {
  tabs: TabState[];
  activeTabIndex: number;
  onSwitchTab: (index: number) => void;
  onCloseTab: (index: number, e?: React.MouseEvent) => void;
  onNewTab: () => void;
}

export const TabBar: React.FC<TabBarProps> = ({
  tabs,
  activeTabIndex,
  onSwitchTab,
  onCloseTab,
  onNewTab,
}) => {
  return (
    <div className="tab-bar" role="tablist">
      {tabs.map((tab, idx) => (
        <div
          key={tab.id}
          className={`tab-item ${idx === activeTabIndex ? "active" : ""}`}
          role="tab"
          aria-selected={idx === activeTabIndex}
          onClick={() => onSwitchTab(idx)}
          title={tab.path}
        >
          <span className="file-icon" style={{ fontSize: "14px" }}>
            <FluentIcon name="folder" size={14} />
          </span>
          <span className="tab-title">{tab.title}</span>
          <button
            className="tab-close-btn"
            aria-label="Close tab"
            title="Close Tab (Ctrl+W)"
            onClick={(e) => onCloseTab(idx, e)}
          >
            <FluentIcon name="close" size={10} />
          </button>
        </div>
      ))}
      <button
        className="new-tab-btn"
        aria-label="New tab"
        title="New Tab (Ctrl+T)"
        onClick={onNewTab}
      >
        <FluentIcon name="add" size={13} />
      </button>
    </div>
  );
};
