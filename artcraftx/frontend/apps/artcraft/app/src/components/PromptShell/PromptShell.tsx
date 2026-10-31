import { ReactNode, useEffect, useRef, useState } from "react";
import { Check } from "lucide-react";
import type { CompletedFile } from "./useComposerTasks";

// The full-window composer frame, styled after the marketing site's AppWindow
// mock: "Prompt" heading, editor filling the window, a signal progress bar
// riding the bottom edge while rendering, and a mono "written to disk"
// receipt when a result lands. Results are auto-saved to the user's download
// directory — the app shows no galleries, files just appear on disk.

const RECEIPT_HIDE_MS = 6000;

interface Receipt {
  text: string;
  key: number;
}

interface PromptShellProps {
  /** Modality glyph rendered before the heading. */
  icon?: ReactNode;
  /** Any task for this page's modality currently in flight. */
  busy: boolean;
  /** Newly-completed files; each gets a "written to disk" receipt once. */
  completed?: CompletedFile[];
  /** The page's promptbox, rendered full-bleed. */
  children: ReactNode;
}

export function PromptShell({
  icon,
  busy,
  completed,
  children,
}: PromptShellProps) {
  const [receipt, setReceipt] = useState<Receipt | null>(null);
  const savedIdsRef = useRef<Set<string>>(new Set());

  // Receipt each completed file exactly once. Every provider's completion
  // path on the Rust side (the ArtCraft poller and the shared third-party
  // completion routine used by Grok, FAL, Midjourney, World Labs and
  // Higgsfield) already saves the results to the user's download directory,
  // named per their filename convention. Downloading them again here would
  // create duplicates (it used to, for every non-ArtCraft provider). The Rust
  // side flashes a toast if a save fails.
  useEffect(() => {
    if (!completed?.length) return;
    const fresh = completed.filter((f) => !savedIdsRef.current.has(f.id));
    if (fresh.length === 0) return;
    fresh.forEach((f) => savedIdsRef.current.add(f.id));

    setReceipt({
      text: fresh.length === 1 ? "1 file" : `${fresh.length} files`,
      key: Date.now(),
    });
  }, [completed]);

  // Auto-fade the receipt.
  useEffect(() => {
    if (!receipt) return;
    const id = window.setTimeout(() => setReceipt(null), RECEIPT_HIDE_MS);
    return () => window.clearTimeout(id);
  }, [receipt]);

  return (
    <div className="relative flex h-[calc(100vh-40px)] w-full flex-col overflow-hidden bg-carbon">
      <div className="flex min-h-0 flex-1 flex-col px-6 py-5">
        <div className="flex shrink-0 items-center gap-2.5">
          {icon && <span className="text-signal">{icon}</span>}
          <h2 className="ax-display text-[17px]">Prompt</h2>
        </div>
        <div className="mt-3 flex min-h-0 flex-1 flex-col">{children}</div>
      </div>

      {receipt && (
        <div
          key={receipt.key}
          className="ax-toast pointer-events-none absolute bottom-16 left-1/2 z-30 -translate-x-1/2"
        >
          <div className="flex items-center gap-3 rounded-ax-md border border-line bg-carbon/95 px-4 py-2.5 shadow-[0_16px_40px_-16px_rgb(0_0_0/0.8)]">
            <span className="grid size-5 shrink-0 place-items-center rounded-ax-sm bg-ok/15 text-ok">
              <Check className="h-2.5 w-2.5" />
            </span>
            <span className="font-mono text-[12.5px] text-bone">
              {receipt.text}
            </span>
            <span className="font-mono text-[11px] uppercase tracking-[0.14em] text-mud">
              written to disk
            </span>
          </div>
        </div>
      )}

      {/* Render progress rides the bottom edge of the window. */}
      <div className="absolute inset-x-0 bottom-0 h-[3px] overflow-hidden bg-bone/5">
        {busy && (
          <div className="ax-progress-bar h-full w-full bg-gradient-to-r from-signal-dim to-signal" />
        )}
      </div>
    </div>
  );
}
