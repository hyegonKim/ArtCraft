import { Modal } from "@storyteller/ui-modal";
import { useEffect, useState } from "react";
import { twMerge } from "tailwind-merge";
import { MiscSettingsPane } from "./panes/MiscSettingsPane";
import { DownloadsSettingsPane } from "./panes/DownloadsSettingsPane";
import { AudioSettingsPane } from "./panes/AudioSettingsPane";
import { AccountSettingsPane } from "./panes/AccountSettings/AccountSettingsPane";
import { AccountBackupSettingsPane } from "./panes/AccountBackupSettingsPane";
import { AboutSettingsPane } from "./panes/AboutSettingsPane";
import { gtagEvent } from "@storyteller/google-analytics";
import { BillingSettingsPane } from "./panes/BillingSettingsPane";
import { AppearanceSettingsPane } from "./panes/AppearanceSettingsPane";
import { Button } from "@storyteller/ui-button";

export type SettingsSection =
  | "general"
  | "downloads"
  | "appearance"
  | "accounts"
  | "backup"
  | "alerts"
  | "about"
  | "billing";

export interface SettingsContentProps {
  globalAccountLogoutCallback: () => void;
  initialSection?: SettingsSection;
}

interface SettingsModalProps extends SettingsContentProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * The settings body (sidebar + panes), independent of any container so it can
 * fill a page or a modal. The host provides the bounded height.
 */
export const SettingsContent = ({
  globalAccountLogoutCallback,
  initialSection = "general",
}: SettingsContentProps) => {
  const [selectedSection, setSelectedSection] =
    useState<SettingsSection>(initialSection);

  const sections = [
    { id: "general" as const, label: "General" },
    { id: "downloads" as const, label: "Downloads" },
    { id: "accounts" as const, label: "Accounts" },
    { id: "backup" as const, label: "Account Backup" },
    { id: "billing" as const, label: "Plan & Credits" },
    { id: "alerts" as const, label: "Alerts" },
    { id: "about" as const, label: "About" },
  ];

  const renderContent = () => {
    switch (selectedSection) {
      case "appearance":
        return <AppearanceSettingsPane />;
      case "alerts":
        return <AudioSettingsPane />;
      case "general":
        return <MiscSettingsPane />;
      case "downloads":
        return <DownloadsSettingsPane />;
      case "accounts":
        return (
          <AccountSettingsPane
            globalAccountLogoutCallback={globalAccountLogoutCallback}
          />
        );
      case "backup":
        return <AccountBackupSettingsPane />;
      case "about":
        return <AboutSettingsPane />;
      case "billing":
        return <BillingSettingsPane />;
    }
  };

  return (
    <>
      <div className="flex h-full">
        {/* Index rail: mono spec labels, signal edge on the open section —
            the marketing site's spec-sheet row language. */}
        <nav
          aria-label="Settings sections"
          className="flex w-52 shrink-0 flex-col border-r border-line px-4 pb-4 pt-5"
        >
          <span className="ax-marker text-mud">Settings</span>
          <div className="mt-4 flex flex-col gap-0.5">
            {sections.map((section) => (
              <button
                key={section.id}
                aria-current={
                  section.id === selectedSection ? "true" : undefined
                }
                className={twMerge(
                  "flex h-8 items-center rounded-ax-sm px-2.5 text-left font-mono text-[11px] uppercase tracking-[0.18em] transition-colors duration-150",
                  "focus-visible:outline focus-visible:outline-1 focus-visible:outline-signal/60",
                  section.id === selectedSection
                    ? "bg-bone/[0.06] text-bone shadow-[inset_2px_0_0_#4d7cfb]"
                    : "text-ash hover:bg-bone/[0.03] hover:text-putty",
                )}
                onClick={() => {
                  gtagEvent("switch_settings_section", {
                    section: section.id,
                  });
                  setSelectedSection(section.id);
                }}
              >
                {section.label}
              </button>
            ))}
          </div>
          <div className="mt-auto pt-4">
            <span className="font-mono text-[10px] uppercase tracking-[0.14em] text-mud">
              ArtCraft-X
            </span>
          </div>
        </nav>

        {/* Section pane: hairline-ruled header row, scrolling body. */}
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-12 shrink-0 items-center justify-between gap-3 border-b border-line px-6">
            <h2 className="ax-display text-[15px]">
              {sections.find((s) => s.id === selectedSection)?.label}
            </h2>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
            <div className="text-sm">{renderContent()}</div>
          </div>
        </div>
      </div>
    </>
  );
};

/** Modal wrapper kept for hosts that still want settings as an overlay. */
export const SettingsModal = ({
  isOpen,
  onClose,
  globalAccountLogoutCallback,
  initialSection = "general",
}: SettingsModalProps) => {
  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      className="max-w-3xl"
      childPadding={false}
    >
      <div className="h-[560px]">
        {/* Remount on open so initialSection applies each time. */}
        <SettingsContent
          key={String(isOpen)}
          globalAccountLogoutCallback={globalAccountLogoutCallback}
          initialSection={initialSection}
        />
      </div>
    </Modal>
  );
};

export default SettingsModal;
