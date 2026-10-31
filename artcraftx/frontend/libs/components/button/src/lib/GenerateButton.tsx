import { Button, ButtonProps } from "./button";
import { Coins } from "lucide-react";
import { twMerge } from "tailwind-merge";
import { Tooltip } from "@storyteller/ui-tooltip";

interface GenerateButtonProps extends ButtonProps {
  credits?: number | null;
}

export const GenerateButton = ({
  credits,
  children,
  className,
  disabled,
  ...props
}: GenerateButtonProps) => {
  return (
    <Button
      className={twMerge(
        "group flex h-[34px] items-center justify-center gap-2 rounded-full px-4",
        className,
      )}
      disabled={disabled}
      {...props}
    >
      <span className="truncate">{children}</span>

      {credits != null && (
        <Tooltip
          content={`${credits} credit${credits !== 1 ? "s" : ""} cost`}
          position="top"
          className="z-50"
        >
          <div
            className={twMerge(
              "flex items-center gap-1.5 opacity-80 group-hover:opacity-100 transition-opacity",
              disabled && "opacity-50",
            )}
          >
            <Coins size="1em" className="text-xs text-white" />
            <span className="text-[13px] font-bold text-white/90">
              {credits}
            </span>
          </div>
        </Tooltip>
      )}
    </Button>
  );
};

export default GenerateButton;
