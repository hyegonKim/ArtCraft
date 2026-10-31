import rhToast, { Toast } from "react-hot-toast";

/** Safety net: a progress toast whose `finished` never arrives (backend
 *  crash, lost event) still goes away eventually. Generous on purpose —
 *  it must outlast the longest step it's used for (Higgsfield's IP check
 *  gives up after 90s). */
const PROGRESS_TOAST_MAX_DURATION_MS = 3 * 60 * 1000;

const BAR_ANIMATION_NAME = "st-progress-toast-slide";

/** Global keyframes for the indeterminate bar. Rendered once by <Toaster />. */
export function ProgressToastStyles() {
  return (
    <style>{`
      @keyframes ${BAR_ANIMATION_NAME} {
        0%   { transform: translateX(-100%); }
        100% { transform: translateX(300%); }
      }
    `}</style>
  );
}

/**
 * Show a toast for a step the user has to wait on. It stays up until one of:
 *  - `dismissProgressToast(id)` is called (the step finished),
 *  - the user clicks its close button,
 *  - the safety timeout elapses.
 *
 * Calling it again with the same `id` updates the message in place.
 */
export function showProgressToast(id: string, message: string): void {
  rhToast.custom((t) => <ProgressToast toast={t} message={message} />, {
    id,
    duration: PROGRESS_TOAST_MAX_DURATION_MS,
  });
}

export function dismissProgressToast(id: string): void {
  rhToast.dismiss(id);
}

interface ProgressToastProps {
  toast: Toast;
  message: string;
}

function ProgressToast({ toast, message }: ProgressToastProps) {
  return (
    <div
      role="status"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        minWidth: 280,
        maxWidth: 380,
        padding: "10px 12px 10px 14px",
        borderRadius: 8,
        background: "#1d4ed8",
        color: "#ffffff",
        boxShadow: "0 3px 10px rgba(0, 0, 0, 0.1), 0 3px 3px rgba(0, 0, 0, 0.05)",
        opacity: toast.visible ? 1 : 0,
        transition: "opacity 200ms ease",
      }}
    >
      <div style={{ display: "flex", alignItems: "flex-start", gap: 10 }}>
        <span style={{ flexShrink: 0 }}>{"ℹ️"}</span>
        <span style={{ flex: 1, lineHeight: 1.35 }}>{message}</span>
        <button
          type="button"
          aria-label="Dismiss"
          onClick={() => rhToast.dismiss(toast.id)}
          style={{
            flexShrink: 0,
            background: "transparent",
            border: "none",
            color: "inherit",
            opacity: 0.8,
            cursor: "pointer",
            fontSize: 18,
            lineHeight: 1,
            padding: "0 2px",
          }}
        >
          {"×"}
        </button>
      </div>
      <div
        style={{
          position: "relative",
          height: 3,
          borderRadius: 2,
          overflow: "hidden",
          background: "rgba(255, 255, 255, 0.25)",
        }}
      >
        <div
          style={{
            position: "absolute",
            top: 0,
            left: 0,
            height: "100%",
            width: "40%",
            borderRadius: 2,
            background: "#ffffff",
            animation: `${BAR_ANIMATION_NAME} 1.4s ease-in-out infinite`,
          }}
        />
      </div>
    </div>
  );
}
