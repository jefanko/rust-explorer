import React from "react";
import type { ModalState } from "../app/types";
import { recycleTargetLabel } from "../lib/recycle";
import { FluentIcon } from "./FluentIcon";

interface ModalDialogProps {
  modal: ModalState;
  modalInputRef: React.RefObject<HTMLInputElement>;
  onClose: () => void;
  onSubmit: (e?: React.FormEvent) => void;
  onValueChange: (val: string) => void;
}

export const ModalDialog: React.FC<ModalDialogProps> = ({
  modal,
  modalInputRef,
  onClose,
  onSubmit,
  onValueChange,
}) => {
  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-container" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">{modal.title}</div>
        <form onSubmit={onSubmit}>
          <div className="modal-body">
            {modal.type === "recycle" ? (
              <div style={{ fontSize: "14px", lineHeight: "1.5" }}>
                Are you sure you want to send{" "}
                <strong>
                  {recycleTargetLabel(modal.value, modal.targetTokens?.length ?? 1)}
                </strong>{" "}
                to the Recycle Bin?
                <div
                  style={{
                    marginTop: "8px",
                    fontSize: "12px",
                    color: "var(--text-muted)",
                  }}
                >
                  Permanent deletion fallback is disabled for safety.
                </div>
              </div>
            ) : (
              <input
                ref={modalInputRef}
                type="text"
                className="modal-input"
                value={modal.value}
                onChange={(e) => onValueChange(e.target.value)}
                placeholder="Enter name..."
              />
            )}
            {modal.error && (
              <div
                style={{
                  color: "var(--error-text)",
                  fontSize: "12px",
                  marginTop: "8px",
                  display: "flex",
                  alignItems: "center",
                  gap: "6px",
                }}
              >
                <FluentIcon name="warning" size={14} />
                <span>{modal.error}</span>
              </div>
            )}
          </div>
          <div className="modal-footer">
            <button
              type="button"
              className="modal-btn modal-btn-secondary"
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="submit"
              className={`modal-btn ${
                modal.type === "recycle"
                  ? "modal-btn-danger"
                  : "modal-btn-primary"
              }`}
            >
              {modal.type === "create_folder"
                ? "Create"
                : modal.type === "rename"
                ? "Rename"
                : "Recycle"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
