import { useEffect, useState } from "react";
import { SoundManager, SoundOption } from "@storyteller/soundboard";
import { Button } from "@storyteller/ui-button";
import { FolderOpen, Play, RotateCcw } from "lucide-react";
import {
  AppSoundEvent,
  AppSoundFile,
  AppSoundPreferences,
  GetAppPreferences,
  PreferenceName,
  ResetSoundPreference,
  SILENT_SOUND,
  UpdateAppPreferences,
  UpdateSoundPreference,
  isCustomWavSound,
} from "@storyteller/tauri-api";
import { open } from "@tauri-apps/plugin-dialog";
import { Select, SelectValue } from "@storyteller/ui-select";
import { Switch } from "@storyteller/ui-switch";
import { Label } from "@storyteller/ui-label";

interface AudioSettingsPaneProps {}

// Rows in display order.
const SOUND_EVENTS: { event: AppSoundEvent; label: string }[] = [
  { event: "delete_file", label: "Delete File Sound" },
  { event: "enqueue_success", label: "Enqueue Success Sound" },
  { event: "enqueue_failure", label: "Enqueue Failure Sound" },
  { event: "generation_success", label: "Generation Success Sound" },
  { event: "generation_failure", label: "Generation Failure Sound" },
];

export const AudioSettingsPane = (args: AudioSettingsPaneProps) => {
  const [sounds, setSounds] = useState<AppSoundPreferences | undefined>(undefined);

  useEffect(() => {
    const fetchData = async () => {
      const prefs = await GetAppPreferences();
      setSounds(prefs.preferences.sounds);
    };
    fetchData();
  }, []);

  const playSounds = sounds?.play_sounds || false;

  const setPlaySounds = async (checked: boolean) => {
    await UpdateAppPreferences({
      preference: PreferenceName.PlaySounds,
      value: checked,
    });
    const prefs = await GetAppPreferences();
    setSounds(prefs.preferences.sounds);
  };

  return (
    <>
      <div className="space-y-4">
        <div className="flex flex-col">
          <Label htmlFor="play-sounds">
            Play Notification Sounds for Events?
          </Label>
          <Switch enabled={playSounds} setEnabled={setPlaySounds} />
        </div>

        {SOUND_EVENTS.map(({ event, label }) => (
          <SoundEventRow
            key={event}
            event={event}
            label={label}
            sound={sounds?.[event]}
            onSoundsChanged={setSounds}
          />
        ))}
      </div>
    </>
  );
};

// ── One event's sound picker ──

interface SoundEventRowProps {
  event: AppSoundEvent;
  label: string;
  sound: AppSoundFile | undefined;
  onSoundsChanged: (sounds: AppSoundPreferences) => void;
}

const SoundEventRow = ({ event, label, sound, onSoundsChanged }: SoundEventRowProps) => {
  const [error, setError] = useState<string | undefined>(undefined);

  // The dropdown lists the presets; when a custom file is set, it also shows
  // that file's full path as the selected entry.
  const customPath = isCustomWavSound(sound) ? sound.custom_wav : undefined;
  const selectedValue = customPath ?? presetValue(sound);
  const options: SoundOption[] = customPath
    ? [{ value: customPath, label: customPath }, ...SoundManager.OPTIONS]
    : SoundManager.OPTIONS;

  const save = async (next: AppSoundFile | undefined) => {
    setError(undefined);
    try {
      const result = await UpdateSoundPreference({ event, sound: next });
      onSoundsChanged(result.sounds);
      SoundManager.playPreview(next, event);
    } catch (err) {
      setError(String(err));
    }
  };

  const resetToDefault = async () => {
    setError(undefined);
    try {
      const result = await ResetSoundPreference(event);
      onSoundsChanged(result.sounds);
      SoundManager.playPreview(result.sounds[event], event);
    } catch (err) {
      setError(String(err));
    }
  };

  // Picking a preset or "None" from the dropdown replaces a custom file too.
  const onSelect = async (val: SelectValue) => {
    const value = String(val);
    if (value === customPath) return; // Re-selected the current custom file.
    await save(value === SILENT_SOUND ? undefined : value);
  };

  const chooseCustomFile = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      title: `Choose a .wav file for: ${label}`,
      filters: [{ name: "WAV audio", extensions: ["wav"] }],
    });
    if (selected === null) {
      return; // User dismissed the dialog.
    }
    await save({ custom_wav: selected });
  };

  const inputId = `${event}-sound`;

  return (
    <div className="space-y-1">
      <Label htmlFor={inputId}>{label}</Label>
      <div className="flex items-center gap-2">
        <Select
          id={inputId}
          value={selectedValue}
          onChange={onSelect}
          options={options}
          className="grow min-w-0"
        />
        <Button
          variant="primary"
          className="w-[40px] h-[40px]"
          icon={Play}
          title="Preview"
          onClick={() => SoundManager.playPreview(sound, event)}
        />
        <Button
          variant="secondary"
          className="w-[40px] h-[40px]"
          icon={FolderOpen}
          title="Use a custom .wav file"
          onClick={chooseCustomFile}
        />
        <Button
          variant="secondary"
          className="w-[40px] h-[40px]"
          icon={RotateCcw}
          title="Reset to default"
          onClick={resetToDefault}
        />
      </div>
      {error && <p className="text-sm text-red-500">{error}</p>}
    </div>
  );
};

// The dropdown value for a preset (or silent) sound.
const presetValue = (sound: AppSoundFile | undefined): string => {
  if (!sound || typeof sound !== "string") {
    return SILENT_SOUND;
  }
  return sound;
};
