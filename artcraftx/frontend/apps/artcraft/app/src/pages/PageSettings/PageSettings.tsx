import { SettingsContent } from "@storyteller/ui-settings-modal";
import { goToApp } from "~/config/appMenu";
import { setLogoutStates } from "~/signals/authentication/utilities";

// Settings as a full page (opened from the titlebar gear) instead of a modal.
// The shared SettingsContent provides the sidebar + panes; this page gives it
// the full-window frame below the titlebar.
export const PageSettings = () => {
  return (
    <div className="h-[calc(100vh-40px)] w-full overflow-hidden bg-carbon">
      <SettingsContent
        globalAccountLogoutCallback={() => {
          setLogoutStates();
          goToApp("IMAGE");
        }}
      />
    </div>
  );
};

export default PageSettings;
