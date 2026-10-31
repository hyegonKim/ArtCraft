import { type LucideIcon } from "lucide-react";
import { Button } from "./button";
import { twMerge } from "tailwind-merge";

interface ToggleButtonProps {
  isActive: boolean;
  icon?: LucideIcon;
  activeIcon?: LucideIcon;
  label?: string;
  onClick: () => void;
  className?: string;
}

export const ToggleButton = ({
  isActive,
  icon,
  activeIcon,
  label,
  onClick,
  className,
}: ToggleButtonProps) => {
  const DisplayIcon = isActive && activeIcon ? activeIcon : icon;
  const hasLabel = Boolean(label);

  return (
    <Button
      className={twMerge(
        // 34px matches the sibling toolbar controls (GenerateButton, the
        // PopoverMenu triggers). Active state is the site's signal-tinted
        // pill; inactive is the standard hairline pill.
        "flex h-[34px] items-center justify-center rounded-lg border text-sm transition-all",
        hasLabel ? "px-3" : "w-[34px]",
        isActive
          ? "border-primary/50 bg-primary/15 text-bone hover:bg-primary/25"
          : "border-line-2 bg-bone/[0.04] text-putty hover:bg-bone/[0.08] hover:text-bone",
        className
      )}
      variant="secondary"
      onClick={onClick}
    >
      <span className="flex items-center gap-2">
        {DisplayIcon && (
          <DisplayIcon
            size="1em"
            className={twMerge("text-base", hasLabel && "text-sm")}
          />
        )}
        {label && (
          <span className="text-sm font-medium text-white/90">{label}</span>
        )}
      </span>
    </Button>
  );
};
