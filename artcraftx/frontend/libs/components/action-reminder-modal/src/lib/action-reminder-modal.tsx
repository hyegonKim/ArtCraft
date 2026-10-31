import { Button } from "@storyteller/ui-button";
import { Modal } from "@storyteller/ui-modal";
import { ReactNode } from "react";
import { LogIn, type LucideIcon } from "lucide-react";
import { twMerge } from "tailwind-merge";

export type ReminderType = "default" | "artcraftLogin";

interface ActionReminderModalProps {
  isOpen: boolean;
  onClose: () => void;
  reminderType?: ReminderType;
  onPrimaryAction: () => void;
  title?: string;
  hideTitle?: boolean;
  message?: ReactNode;
  children?: ReactNode;
  primaryActionText?: string;
  secondaryActionText?: string;
  onSecondaryAction?: () => void;
  isLoading?: boolean;
  openAiLogo?: string;
  modalClassName?: string;
  primaryActionIcon?: LucideIcon;
  primaryActionBtnClassName?: string;
}

export function ActionReminderModal({
  isOpen,
  onClose,
  reminderType = "default",
  onPrimaryAction,
  title: customTitle,
  hideTitle = false,
  message: customMessage,
  children: customChildren,
  primaryActionText: customPrimaryActionText,
  secondaryActionText: customSecondaryActionText,
  onSecondaryAction,
  isLoading = false,
  modalClassName: customModalClassName = "",
  primaryActionIcon: customPrimaryActionIcon = LogIn,
  primaryActionBtnClassName: customPrimaryActionBtnClassName = "",
}: ActionReminderModalProps) {
  let titleText: string | undefined;
  let messageContent: ReactNode;
  let primaryActionBtnText: string;
  let primaryActionIcon: LucideIcon = LogIn;
  let primaryActionBtnClassName = "";

  if (!hideTitle) {
    switch (reminderType) {
      case "artcraftLogin":
        titleText = customTitle || "Login to ArtCraft";
        break;
      default:
        titleText = customTitle || "Action Required";
        break;
    }
  } else {
    titleText = undefined;
  }

  switch (reminderType) {
    case "artcraftLogin":
      messageContent = customMessage || (
        <p className="text-sm text-white/70">
          Please log in or sign up to ArtCraft to proceed. This will allow you
          to save your work and access all features.
        </p>
      );
      primaryActionBtnText = customPrimaryActionText || "Login / Sign Up";
      primaryActionIcon = customPrimaryActionIcon || LogIn;
      primaryActionBtnClassName = customPrimaryActionBtnClassName || "";
      break;
    default:
      messageContent = customMessage || (
        <p className="text-sm text-white/70">
          Please complete the required action.
        </p>
      );
      primaryActionBtnText = customPrimaryActionText || "Proceed";
      primaryActionIcon = customPrimaryActionIcon || LogIn;
      primaryActionBtnClassName = customPrimaryActionBtnClassName || "";
      break;
  }

  const effectiveSecondaryAction = onSecondaryAction || onClose;
  const effectiveSecondaryActionText = customSecondaryActionText || "Cancel";

  let modalSpecificClasses = "";

  const finalModalClassName =
    `${customModalClassName} ${modalSpecificClasses}`.trim();

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={titleText}
      className={finalModalClassName}
    >
      <div>
        {customChildren ? (
          <div className="space-y-4">{customChildren}</div>
        ) : (
          <div className="space-y-4">{messageContent}</div>
        )}

        <div className="mt-6 flex flex-col sm:flex-row-reverse gap-3">
          <Button
            onClick={onPrimaryAction}
            loading={isLoading}
            disabled={isLoading}
            icon={primaryActionIcon}
            className={twMerge("w-full sm:w-auto", primaryActionBtnClassName)}
          >
            {primaryActionBtnText}
          </Button>
          {(onSecondaryAction || customSecondaryActionText) && (
            <Button
              variant="secondary"
              onClick={effectiveSecondaryAction}
              disabled={isLoading}
              className="w-full sm:w-auto"
            >
              {effectiveSecondaryActionText}
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
}

export default ActionReminderModal;
