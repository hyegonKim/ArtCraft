import { ArrowUp, Coins, LoaderCircle } from "lucide-react";
import { ButtonHTMLAttributes } from "react";
import { twMerge } from "tailwind-merge";
import { Tooltip } from "@storyteller/ui-tooltip";

interface GenerateIconButtonProps extends Omit<
  ButtonHTMLAttributes<HTMLButtonElement>,
  "children"
> {
  credits?: number | null;
  loading?: boolean;
}

/**
 * Compact generate control: credit cost as plain text next to a circular
 * arrow-up button. Desktop promptboxes use this; the mobile form keeps the
 * labeled `GenerateButton` pill.
 */
export const GenerateIconButton = ({
  credits,
  loading,
  className,
  disabled,
  ...rest
}: GenerateIconButtonProps) => {
  const isDisabled = disabled || loading;

  return (
    <div className={twMerge("flex shrink-0 items-center gap-3.5", className)}>
      {credits != null && (
        <Tooltip
          content={`${credits} credit${credits !== 1 ? "s" : ""} cost`}
          position="top"
          className="z-50"
        >
          <span
            className={twMerge(
              "flex items-center gap-1.5 ms-1.5 text-[13px] font-semibold tabular-nums text-base-fg/80 transition-opacity",
              isDisabled && "opacity-50",
            )}
          >
            <Coins className="h-3 w-3" />
            {credits}
          </span>
        </Tooltip>
      )}

      <button
        type="button"
        className="flex h-9 items-center justify-center gap-1.5 rounded-full bg-primary px-4 text-[13px] font-medium text-white transition-all duration-150 hover:bg-primary-400 active:scale-95 disabled:cursor-not-allowed disabled:opacity-40 disabled:active:scale-100"
        disabled={isDisabled}
        {...rest}
      >
        Create
        {loading ? (
          <LoaderCircle className="h-4 w-4 animate-spin" />
        ) : (
          <ArrowUp className="h-4 w-4" />
        )}
      </button>
    </div>
  );
};

export default GenerateIconButton;
