import React from "react";

export type FluentIconName =
  | "folder"
  | "folder-add"
  | "file"
  | "file-text"
  | "file-image"
  | "file-video"
  | "file-music"
  | "file-archive"
  | "file-code"
  | "file-exe"
  | "desktop"
  | "downloads"
  | "documents"
  | "pictures"
  | "music"
  | "videos"
  | "star"
  | "drive"
  | "cut"
  | "copy"
  | "paste"
  | "rename"
  | "delete"
  | "refresh"
  | "search"
  | "eye"
  | "eye-off"
  | "activity"
  | "theme-sun"
  | "theme-moon"
  | "theme-system"
  | "close"
  | "chevron-right"
  | "chevron-down"
  | "arrow-left"
  | "arrow-right"
  | "arrow-up"
  | "sort-asc"
  | "sort-desc"
  | "add"
  | "warning";

interface FluentIconProps {
  name: FluentIconName;
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}

export function FluentIcon({ name, size = 16, className = "", style }: FluentIconProps) {
  const s = size;

  switch (name) {
    // Folders & Items
    case "folder":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="currentColor" className={`fluent-icon ${className}`} style={style}>
          <path d="M1.5 3A1.5 1.5 0 0 0 0 4.5v7A1.5 1.5 0 0 0 1.5 13h13a1.5 1.5 0 0 0 1.5-1.5v-6A1.5 1.5 0 0 0 14.5 4H7.414L5.707 2.293A1 1 0 0 0 5 2H1.5a1.5 1.5 0 0 0-1 .382V4.5A.5.5 0 0 1 1 4h13.5a.5.5 0 0 1 .5.5v6a.5.5 0 0 1-.5.5h-13a.5.5 0 0 1-.5-.5v-6A.5.5 0 0 1 1 4V3z" fillRule="evenodd" />
        </svg>
      );
    case "folder-add":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="currentColor" className={`fluent-icon ${className}`} style={style}>
          <path d="M7 3.5 5.5 2H1.5A1.5 1.5 0 0 0 0 3.5v9A1.5 1.5 0 0 0 1.5 14h6.75a4.48 4.48 0 0 1-.25-1.5c0-.52.09-1.02.25-1.5H1.5a.5.5 0 0 1-.5-.5v-7h13a.5.5 0 0 1 .5.5v3.13c.53.22 1.02.53 1.43.91.04-.18.07-.36.07-.54v-5A1.5 1.5 0 0 0 14.5 4H7.5L7 3.5z" />
          <path d="M12.5 9a3.5 3.5 0 1 1 0 7 3.5 3.5 0 0 1 0-7zm0 1.5a.5.5 0 0 0-.5.5v1h-1a.5.5 0 0 0 0 1h1v1a.5.5 0 0 0 1 0v-1h1a.5.5 0 0 0 0-1h-1v-1a.5.5 0 0 0-.5-.5z" />
        </svg>
      );
    case "file":
    case "file-text":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M3.5 1.5h6l3.5 3.5v9.5a1 1 0 0 1-1 1h-8.5a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1z" />
          <path d="M9.5 1.5v3.5h3.5" />
          <line x1="5.5" y1="8" x2="10.5" y2="8" />
          <line x1="5.5" y1="10.5" x2="9" y2="10.5" />
        </svg>
      );
    case "file-image":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="2" y="2" width="12" height="12" rx="1.5" />
          <circle cx="5.5" cy="5.5" r="1.2" fill="currentColor" stroke="none" />
          <path d="m14 11-4-4-5 5" />
          <path d="m10 10 2 2" />
        </svg>
      );
    case "file-video":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" />
          <polygon points="6,5.5 11,8 6,10.5" fill="currentColor" stroke="none" />
        </svg>
      );
    case "file-music":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M6 12.5a2 2 0 1 1-2-2 2 2 0 0 1 2 2zm7-2a2 2 0 1 1-2-2 2 2 0 0 1 2 2z" />
          <path d="M6 10.5v-7l7-2v7" />
          <line x1="6" y1="5.5" x2="13" y2="3.5" />
        </svg>
      );
    case "file-archive":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="2.5" y="1.5" width="11" height="13" rx="1" />
          <line x1="8" y1="1.5" x2="8" y2="7.5" strokeDasharray="1.5 1.5" />
          <rect x="6.5" y="7.5" width="3" height="2.5" rx="0.5" fill="currentColor" />
        </svg>
      );
    case "file-code":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M3.5 1.5h6l3.5 3.5v9.5a1 1 0 0 1-1 1h-8.5a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1z" />
          <path d="m6 8-1.5 1.5L6 11" />
          <path d="m9 8 1.5 1.5L9 11" />
        </svg>
      );
    case "file-exe":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="2" y="2" width="12" height="12" rx="1.5" />
          <path d="M5 6.5 7.5 9 5 11.5" />
          <line x1="8.5" y1="11.5" x2="11" y2="11.5" />
        </svg>
      );

    // Sidebar & Navigation Items
    case "desktop":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="1.5" y="2" width="13" height="9" rx="1" />
          <line x1="5.5" y1="14" x2="10.5" y2="14" />
          <line x1="8" y1="11" x2="8" y2="14" />
        </svg>
      );
    case "downloads":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M8 2v8m0 0-3-3m3 3 3-3" />
          <path d="M2.5 10.5v3a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-3" />
        </svg>
      );
    case "documents":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M3.5 1.5h6l3.5 3.5v9.5a1 1 0 0 1-1 1h-8.5a1 1 0 0 1-1-1v-12a1 1 0 0 1 1-1z" />
          <path d="M9.5 1.5v3.5h3.5" />
          <line x1="5.5" y1="7" x2="10.5" y2="7" />
          <line x1="5.5" y1="9.5" x2="10.5" y2="9.5" />
          <line x1="5.5" y1="12" x2="8.5" y2="12" />
        </svg>
      );
    case "pictures":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="2" y="2" width="12" height="12" rx="1.5" />
          <circle cx="5.5" cy="5.5" r="1.2" />
          <path d="m14 11-4-4-5 5" />
          <path d="m10 10 2 2" />
        </svg>
      );
    case "music":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <circle cx="4.5" cy="11.5" r="2" />
          <circle cx="11.5" cy="9.5" r="2" />
          <path d="M6.5 11.5V3.5l7-2v8" />
        </svg>
      );
    case "videos":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="1.5" y="3" width="10" height="10" rx="1" />
          <polygon points="11.5,6.5 14.5,4.5 14.5,11.5 11.5,9.5" fill="currentColor" stroke="none" />
        </svg>
      );
    case "star":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="currentColor" className={`fluent-icon ${className}`} style={style}>
          <path d="m8 1.5 2 4.5 4.8.5-3.6 3.2 1.1 4.8L8 12.2l-4.3 2.3 1.1-4.8-3.6-3.2 4.8-.5 2-4.5z" />
        </svg>
      );
    case "drive":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="1.5" y="4.5" width="13" height="7" rx="1.5" />
          <circle cx="11.5" cy="8" r="0.8" fill="currentColor" />
          <line x1="3.5" y1="8" x2="6.5" y2="8" />
        </svg>
      );

    // Command Bar Actions
    case "cut":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <circle cx="4.5" cy="4.5" r="2" />
          <circle cx="4.5" cy="11.5" r="2" />
          <path d="m6 6 8 8" />
          <path d="m6 10 2-2" />
          <path d="m10 6 4-4" />
        </svg>
      );
    case "copy":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="4.5" y="4.5" width="9" height="10" rx="1" />
          <path d="M11.5 4.5V2.5a1 1 0 0 0-1-1h-8a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h2" />
        </svg>
      );
    case "paste":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="2.5" y="4" width="11" height="10.5" rx="1.5" />
          <path d="M5.5 4V2.5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1V4" />
          <line x1="5.5" y1="7.5" x2="10.5" y2="7.5" />
          <line x1="5.5" y1="10" x2="8.5" y2="10" />
        </svg>
      );
    case "rename":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="m11 2.5 2.5 2.5L4.5 14H2v-2.5L11 2.5z" />
          <line x1="9.5" y1="4" x2="12" y2="6.5" />
        </svg>
      );
    case "delete":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M2.5 4h11" />
          <path d="M5.5 4V2.5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1V4" />
          <path d="M3.5 4v9a1.5 1.5 0 0 0 1.5 1.5h6a1.5 1.5 0 0 0 1.5-1.5V4" />
          <line x1="6.5" y1="7" x2="6.5" y2="11.5" />
          <line x1="9.5" y1="7" x2="9.5" y2="11.5" />
        </svg>
      );
    case "refresh":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M13.5 8A5.5 5.5 0 1 1 12 4.1L13.5 2.5" />
          <path d="M13.5 6V2.5H10" />
        </svg>
      );
    case "search":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <circle cx="7" cy="7" r="4.5" />
          <line x1="10.5" y1="10.5" x2="14" y2="14" />
        </svg>
      );
    case "eye":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M1.5 8s2.5-4.5 6.5-4.5 6.5 4.5 6.5 4.5-2.5 4.5-6.5 4.5-6.5-4.5-6.5-4.5z" />
          <circle cx="8" cy="8" r="2" />
        </svg>
      );
    case "eye-off":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M1.5 1.5 14.5 14.5" />
          <path d="M6.5 6.6a2 2 0 0 0 2.9 2.9" />
          <path d="M3.7 4.1C2.5 5.2 1.5 8 1.5 8s2.5 4.5 6.5 4.5c1.4 0 2.6-.5 3.6-1.2" />
          <path d="M10.3 5.3A6.2 6.2 0 0 0 8 3.5C4 3.5 1.5 8 1.5 8s1 1.7 2.2 2.6" />
        </svg>
      );
    case "activity":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <polyline points="1.5,8.5 4.5,8.5 6.5,3.5 9.5,13.5 11.5,8.5 14.5,8.5" />
        </svg>
      );
    case "theme-sun":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <circle cx="8" cy="8" r="3" />
          <line x1="8" y1="1.5" x2="8" y2="3" />
          <line x1="8" y1="13" x2="8" y2="14.5" />
          <line x1="1.5" y1="8" x2="3" y2="8" />
          <line x1="13" y1="8" x2="14.5" y2="8" />
          <line x1="3.4" y1="3.4" x2="4.5" y2="4.5" />
          <line x1="11.5" y1="11.5" x2="12.6" y2="12.6" />
          <line x1="3.4" y1="12.6" x2="4.5" y2="11.5" />
          <line x1="11.5" y1="4.5" x2="12.6" y2="3.4" />
        </svg>
      );
    case "theme-moon":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M13.5 10.2A6 6 0 1 1 5.8 2.5a5 5 0 0 0 7.7 7.7z" />
        </svg>
      );
    case "theme-system":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <rect x="2" y="2" width="12" height="9" rx="1.5" />
          <line x1="5.5" y1="14" x2="10.5" y2="14" />
          <line x1="8" y1="11" x2="8" y2="14" />
        </svg>
      );
    case "close":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <line x1="3.5" y1="3.5" x2="12.5" y2="12.5" />
          <line x1="12.5" y1="3.5" x2="3.5" y2="12.5" />
        </svg>
      );
    case "chevron-right":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <polyline points="6,3.5 10.5,8 6,12.5" />
        </svg>
      );
    case "chevron-down":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <polyline points="3.5,6 8,10.5 12.5,6" />
        </svg>
      );
    case "arrow-left":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <line x1="13.5" y1="8" x2="2.5" y2="8" />
          <polyline points="7,3.5 2.5,8 7,12.5" />
        </svg>
      );
    case "arrow-right":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <line x1="2.5" y1="8" x2="13.5" y2="8" />
          <polyline points="9,3.5 13.5,8 9,12.5" />
        </svg>
      );
    case "arrow-up":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <line x1="8" y1="13.5" x2="8" y2="2.5" />
          <polyline points="3.5,7 8,2.5 12.5,7" />
        </svg>
      );
    case "sort-asc":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M4 13V3m0 0L1.5 5.5M4 3l2.5 2.5" />
          <line x1="9" y1="4" x2="14" y2="4" />
          <line x1="9" y1="8" x2="12.5" y2="8" />
          <line x1="9" y1="12" x2="11" y2="12" />
        </svg>
      );
    case "sort-desc":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M4 3v10m0 0L1.5 10.5M4 13l2.5-2.5" />
          <line x1="9" y1="4" x2="14" y2="4" />
          <line x1="9" y1="8" x2="12.5" y2="8" />
          <line x1="9" y1="12" x2="11" y2="12" />
        </svg>
      );
    case "add":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="currentColor" className={`fluent-icon ${className}`} style={style}>
          <path d="M8 2.5a.75.75 0 0 1 .75.75v4h4a.75.75 0 0 1 0 1.5h-4v4a.75.75 0 0 1-1.5 0v-4h-4a.75.75 0 0 1 0-1.5h4v-4A.75.75 0 0 1 8 2.5z" />
        </svg>
      );
    case "warning":
      return (
        <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" className={`fluent-icon ${className}`} style={style}>
          <path d="M7.13 2.5a1 1 0 0 1 1.74 0l5.5 10A1 1 0 0 1 13.5 14h-11a1 1 0 0 1-.87-1.5l5.5-10z" />
          <line x1="8" y1="6" x2="8" y2="9.5" />
          <circle cx="8" cy="11.5" r="0.75" fill="currentColor" stroke="none" />
        </svg>
      );
    default:
      return null;
  }
}

export function getFluentFileIcon(entry: { kind: string; extension: string }): FluentIconName {
  if (entry.kind === "directory") return "folder";
  const ext = entry.extension.toLowerCase();
  if (["png", "jpg", "jpeg", "gif", "bmp", "webp", "svg", "ico"].includes(ext)) return "file-image";
  if (["mp4", "mkv", "avi", "mov", "wmv", "webm"].includes(ext)) return "file-video";
  if (["mp3", "wav", "flac", "m4a", "ogg", "wma"].includes(ext)) return "file-music";
  if (["zip", "rar", "7z", "tar", "gz", "bz2", "xz"].includes(ext)) return "file-archive";
  if (["exe", "msi", "bat", "cmd", "ps1"].includes(ext)) return "file-exe";
  if (["rs", "ts", "tsx", "js", "jsx", "json", "toml", "html", "css", "c", "cpp", "h", "hpp", "py"].includes(ext)) return "file-code";
  if (["txt", "md", "pdf", "docx", "xlsx", "pptx"].includes(ext)) return "file-text";
  return "file";
}

export function getFluentKnownFolderIcon(name: string): FluentIconName {
  const lower = name.toLowerCase();
  if (lower.includes("desktop")) return "desktop";
  if (lower.includes("download")) return "downloads";
  if (lower.includes("document")) return "documents";
  if (lower.includes("picture") || lower.includes("photo")) return "pictures";
  if (lower.includes("music")) return "music";
  if (lower.includes("video") || lower.includes("movie")) return "videos";
  return "folder";
}
