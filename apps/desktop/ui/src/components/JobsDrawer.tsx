import React from "react";
import type { JobSummary } from "../bridge/types";
import { FluentIcon } from "./FluentIcon";

interface JobsDrawerProps {
  jobs: JobSummary[];
  onClose: () => void;
}

export const JobsDrawer: React.FC<JobsDrawerProps> = ({ jobs, onClose }) => {
  return (
    <div className="jobs-drawer">
      <div className="jobs-header">
        <span>Operation Jobs ({jobs.length})</span>
        <div className="jobs-header-actions">
          <button className="jobs-close-btn" onClick={onClose} title="Close">
            <FluentIcon name="close" size={10} />
          </button>
        </div>
      </div>
      <div className="jobs-list">
        {jobs.length === 0 ? (
          <div className="jobs-empty-note">No recent operation jobs</div>
        ) : (
          jobs.map((job) => (
            <div key={job.id} className="job-card">
              <div className="job-card-header">
                <span className="job-card-title">{job.kind.replace("_", " ")}</span>
                <span className={`job-badge ${job.state}`}>{job.state}</span>
              </div>
              <div className="job-card-details">
                <span>
                  {job.completed_items} succeeded · {job.failed_items} failed ·{" "}
                  {job.canceled_items} canceled · {job.skipped_items} skipped
                </span>
                <span>
                  {new Date(job.created_at_epoch * 1000).toLocaleTimeString([], {
                    hour: "2-digit",
                    minute: "2-digit",
                    second: "2-digit",
                  })}
                </span>
              </div>
              {job.error_message && (
                <div
                  className="job-card-error"
                  style={{ display: "flex", alignItems: "center", gap: "6px" }}
                >
                  <FluentIcon name="warning" size={14} />
                  <span>{job.error_message}</span>
                </div>
              )}
              {job.item_outcomes.length > 0 && (
                <details className="job-item-outcomes">
                  <summary>Item outcomes ({job.item_outcomes.length})</summary>
                  <ul>
                    {job.item_outcomes.slice(0, 100).map((outcome, index) => (
                      <li
                        key={`${job.id}-${index}`}
                        title={outcome.error_message || undefined}
                      >
                        <strong>{outcome.status}</strong> — {outcome.item_display}
                        {(outcome.actual_destination_display ||
                          outcome.requested_destination_display) && (
                          <>
                            {" "}
                            →{" "}
                            {outcome.actual_destination_display ||
                              outcome.requested_destination_display}
                          </>
                        )}
                      </li>
                    ))}
                  </ul>
                  {job.item_outcomes.length > 100 && (
                    <p>
                      Showing first 100 of {job.item_outcomes.length} outcomes.
                    </p>
                  )}
                </details>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
};
