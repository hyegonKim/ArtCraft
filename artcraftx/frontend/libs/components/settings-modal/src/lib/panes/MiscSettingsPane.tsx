import {
  PromptPreferenceName,
  UpdatePromptPreference,
  useAppPreferencesStore,
  useEnterToGenerate,
} from "@storyteller/tauri-api";
import { Switch } from "@storyteller/ui-switch";
import { SettingsRow } from "./SettingsRow";

interface MiscSettingsPaneProps {}

export const MiscSettingsPane = (args: MiscSettingsPaneProps) => {
  const enterToGenerate = useEnterToGenerate();
  const refreshPreferences = useAppPreferencesStore((s) => s.refresh);

  const setEnterToGenerate = async (enabled: boolean) => {
    try {
      await UpdatePromptPreference({
        preference: PromptPreferenceName.EnterToGenerate,
        value: enabled,
      });
    } catch (err) {
      console.error("Could not save Enter-to-generate preference:", err);
    }
    // The backend also broadcasts app_preferences_changed_event; refreshing
    // here just makes the switch settle without waiting on it.
    await refreshPreferences();
  };

  return (
    <div className="text-base-fg">
      <SettingsRow
        title="Enter to generate"
        description="When on, Enter submits the prompt and Shift+Enter adds a new line. When off, both add a new line — use the button to submit."
      >
        <Switch enabled={enterToGenerate} setEnabled={setEnterToGenerate} />
      </SettingsRow>
    </div>
  );
};
