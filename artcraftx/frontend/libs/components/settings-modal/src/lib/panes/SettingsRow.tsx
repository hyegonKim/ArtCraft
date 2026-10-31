import { ReactNode } from "react";
import { twMerge } from "tailwind-merge";

// Spec-sheet row primitives shared by the settings panes: full-width rows
// with the label block on the left, the control on the right, separated by
// hairlines — the marketing site's spec-list language.

interface SettingsRowProps {
  title: ReactNode;
  description?: ReactNode;
  /** Control rendered on the row's right edge. */
  children?: ReactNode;
  className?: string;
}

export const SettingsRow = ({
  title,
  description,
  children,
  className,
}: SettingsRowProps) => (
  <div
    className={twMerge(
      "flex items-center justify-between gap-8 border-b border-line py-3.5 last:border-b-0",
      className,
    )}
  >
    <div className="flex min-w-0 flex-col gap-1">
      <span className="text-[13.5px] font-medium text-bone">{title}</span>
      {description && (
        <span className="max-w-xl text-xs leading-relaxed text-ash">
          {description}
        </span>
      )}
    </div>
    <div className="shrink-0">{children}</div>
  </div>
);

/** Full-width variant: the control block sits under the label instead of on
 *  the right edge (paths, button groups, lists). */
export const SettingsBlock = ({
  title,
  description,
  children,
  className,
}: SettingsRowProps) => (
  <div
    className={twMerge(
      "flex flex-col gap-2.5 border-b border-line py-3.5 last:border-b-0",
      className,
    )}
  >
    <div className="flex flex-col gap-1">
      <span className="text-[13.5px] font-medium text-bone">{title}</span>
      {description && (
        <span className="max-w-xl text-xs leading-relaxed text-ash">
          {description}
        </span>
      )}
    </div>
    {children}
  </div>
);
