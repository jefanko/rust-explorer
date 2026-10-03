import { useState } from "react";

export default function App() {
  const [activeTab, setActiveTab] = useState("tab-1");

  return (
    <div className="app-container">
      <header className="app-header">
        <div className="tab-bar" role="tablist">
          <div
            className={`tab-item ${activeTab === "tab-1" ? "active" : ""}`}
            role="tab"
            aria-selected={activeTab === "tab-1"}
            onClick={() => setActiveTab("tab-1")}
          >
            <span>Home</span>
          </div>
          <button className="new-tab-btn" aria-label="New tab" title="New Tab (Ctrl+T)">
            +
          </button>
        </div>
        <div className="nav-toolbar">
          <div className="nav-buttons">
            <button aria-label="Back" title="Back (Alt+Left)">←</button>
            <button aria-label="Forward" title="Forward (Alt+Right)">→</button>
            <button aria-label="Up" title="Up to Parent (Alt+Up)">↑</button>
          </div>
          <div className="address-bar-container">
            <input
              type="text"
              className="address-input"
              defaultValue="This PC"
              aria-label="Address"
            />
          </div>
          <div className="search-container">
            <input
              type="text"
              className="search-input"
              placeholder="Search (Ctrl+F)"
              aria-label="Search"
            />
          </div>
        </div>
      </header>

      <div className="app-body">
        <aside className="sidebar">
          <section className="sidebar-section">
            <h3>Favorites</h3>
            <ul>
              <li>Documents</li>
              <li>Downloads</li>
              <li>Pictures</li>
            </ul>
          </section>
          <section className="sidebar-section">
            <h3>Drives</h3>
            <ul>
              <li>Local Disk (C:)</li>
            </ul>
          </section>
        </aside>

        <main className="content-pane" role="region" aria-label="Folder contents">
          <div className="file-table-header">
            <span className="col col-name">Name</span>
            <span className="col col-type">Type</span>
            <span className="col col-size">Size</span>
            <span className="col col-date">Date modified</span>
          </div>
          <div className="file-table-body">
            <div className="empty-state">
              <p>Welcome to Rust Explorer (MVP Milestone M0 Baseline)</p>
            </div>
          </div>
        </main>
      </div>

      <footer className="app-statusbar">
        <span>0 items</span>
        <span className="status-spacer"></span>
        <span>Rust Engine: Connected</span>
      </footer>
    </div>
  );
}
