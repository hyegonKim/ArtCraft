import { CredentialsManager } from "./CredentialsManager";

interface AccountSettingsPaneProps {
  // Kept for call-site compatibility; the old account blocks that used this
  // are gone now that credentials are file-based.
  globalAccountLogoutCallback: () => void;
}

export const AccountSettingsPane = (_props: AccountSettingsPaneProps) => {
  return <CredentialsManager />;
};
