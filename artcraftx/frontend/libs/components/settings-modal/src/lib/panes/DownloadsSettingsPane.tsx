import { useEffect, useState } from "react";
import { Button } from "@storyteller/ui-button";
import {
  AppPreferencesPayload,
  CustomDirectory,
  CustomFilenameFormat,
  GetAppPreferences,
  PreferredDownloadFilename,
  SystemDirectory,
} from "@storyteller/tauri-api";
import { PreferenceName, UpdateAppPreferences } from "@storyteller/tauri-api";
import { open } from "@tauri-apps/plugin-dialog";
import { DownloadDirectoryReveal } from "@storyteller/tauri-api";
import { Folder, RotateCcw, Search } from "lucide-react";
import { Select, SelectValue } from "@storyteller/ui-select";
import { useTauriPlatform } from "@storyteller/tauri-utils";
import { SettingsBlock } from "./SettingsRow";

const ARTCRAFT_CONVENTION = "artcraft_convention";
const CUSTOM = "custom";

const DEFAULT_CUSTOM_FORMAT = "{model}_{date}";

// Characters the backend rejects in custom formats.
const UNSAFE_FORMAT_CHARACTERS = /[/\\'"`%<>|:*?]/;

const FILENAME_OPTIONS = [
  { value: ARTCRAFT_CONVENTION, label: "ArtCraft convention" },
  { value: CUSTOM, label: "Custom format" },
];

interface DownloadsSettingsPaneProps {}

export const DownloadsSettingsPane = (args: DownloadsSettingsPaneProps) => {
  const [preferences, setPreferences] = useState<
    AppPreferencesPayload | undefined
  >(undefined);

  const platform = useTauriPlatform();

  // Match each OS's file-manager vocabulary.
  const chooseLabel =
    platform === "linux" ? "Choose directory" : "Choose folder";
  const showLabel =
    platform === "windows"
      ? "Show in Explorer"
      : platform === "macos"
        ? "Show in Finder"
        : "Show directory";

  const [filenameMode, setFilenameMode] = useState<string>(ARTCRAFT_CONVENTION);
  const [customFormat, setCustomFormat] = useState<string>(DEFAULT_CUSTOM_FORMAT);
  const [formatError, setFormatError] = useState<string | null>(null);

  const applyPreferences = (prefs: AppPreferencesPayload) => {
    setPreferences(prefs);
    const filename: PreferredDownloadFilename | undefined =
      prefs.downloads.preferred_download_filename;
    if (filename && typeof filename === "object" && "custom_format" in filename) {
      setFilenameMode(CUSTOM);
      setCustomFormat(filename.custom_format);
    } else {
      setFilenameMode(ARTCRAFT_CONVENTION);
    }
  };

  useEffect(() => {
    const fetchData = async () => {
      const prefs = await GetAppPreferences();
      applyPreferences(prefs.preferences);
    };
    fetchData();
  }, []);

  const reloadPreferences = async () => {
    const prefs = await GetAppPreferences();
    applyPreferences(prefs.preferences);
  };

  // NB: This might be a complex type.
  const outerDownloadObject = preferences?.downloads.preferred_download_directory || {};
  const downloadDirectory =
    "custom" in outerDownloadObject
      ? (outerDownloadObject.custom as string)
      : "";
  const currentDownloadLabel =
    "system" in outerDownloadObject
      ? "System Download Directory"
      : downloadDirectory;

  const openDirectoryPicker = async () => {
    let directory = await open({
      multiple: false,
      directory: true,
      defaultPath: downloadDirectory || undefined,
    });
    if (directory === null) {
      return; // User dismissed the dialog choice
    }
    await UpdateAppPreferences({
      preference: PreferenceName.PreferredDownloadDirectory,
      value: {
        custom: directory,
      } as CustomDirectory,
    });
    await reloadPreferences();
  };

  const clearDirectory = async () => {
    await UpdateAppPreferences({
      preference: PreferenceName.PreferredDownloadDirectory,
      value: {
        system: "downloads",
      } as SystemDirectory,
    });
    await reloadPreferences();
  };

  const showDirectory = async () => {
    await DownloadDirectoryReveal();
  };

  const saveFilenamePreference = async (
    value: PreferredDownloadFilename,
  ): Promise<boolean> => {
    try {
      await UpdateAppPreferences({
        preference: PreferenceName.PreferredDownloadFilename,
        value,
      });
      await reloadPreferences();
      return true;
    } catch (error) {
      setFormatError(String(error));
      return false;
    }
  };

  const changeFilenameMode = async (mode: string) => {
    setFilenameMode(mode);
    setFormatError(null);
    if (mode === ARTCRAFT_CONVENTION) {
      await saveFilenamePreference(ARTCRAFT_CONVENTION);
    }
    // Custom mode saves on "Save format" so half-typed formats never persist.
  };

  const saveCustomFormat = async () => {
    const format = customFormat.trim();
    if (!format) {
      setFormatError("Format cannot be empty");
      return;
    }
    if (UNSAFE_FORMAT_CHARACTERS.test(format) || format.includes("..")) {
      setFormatError("Format cannot contain slashes, quotes, or other unsafe characters");
      return;
    }
    setFormatError(null);
    await saveFilenamePreference({ custom_format: format } as CustomFilenameFormat);
  };

  return (
    <div className="text-base-fg">
      <SettingsBlock
        title="Download directory"
        description="Generated files are written straight to this folder."
      >
        <div className="w-full overflow-x-auto rounded-ax-sm border border-line bg-well/60 px-3 py-2 font-mono text-[12px] text-putty">
          {currentDownloadLabel}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="secondary" onClick={openDirectoryPicker}>
            <Folder className="h-3.5 w-3.5" />
            {chooseLabel}
          </Button>
          <Button variant="secondary" onClick={showDirectory}>
            <Search className="h-3.5 w-3.5" />
            {showLabel}
          </Button>
          <Button variant="ghost" onClick={clearDirectory}>
            <RotateCcw className="h-3.5 w-3.5" />
            Use default
          </Button>
        </div>
      </SettingsBlock>
      <SettingsBlock
        title="Filename convention"
        description="How downloaded files are named. The ArtCraft convention is {model}_{date}.{ext}, with a batch index when a generation produces several files."
      >
        <div className="max-w-xs">
          <Select
            options={FILENAME_OPTIONS}
            value={filenameMode}
            onChange={(val: SelectValue) => changeFilenameMode(val as string)}
          />
        </div>
        {filenameMode === CUSTOM && (
          <div className="flex flex-col gap-2">
            <div className="flex flex-wrap items-center gap-2">
              <input
                type="text"
                value={customFormat}
                onChange={(e) => setCustomFormat(e.target.value)}
                placeholder={DEFAULT_CUSTOM_FORMAT}
                spellCheck={false}
                className="w-72 rounded-ax-sm border border-line bg-well/60 px-3 py-2 font-mono text-[12px] text-base-fg outline-none focus:border-putty"
              />
              <Button variant="secondary" onClick={saveCustomFormat}>
                Save format
              </Button>
            </div>
            <p className="text-[12px] text-mud">
              {"Tokens: {model}, {date}, {YYYY}, {YY}, {MM}, {DD}, {HH}, {mm}, {SS}, {batch_index}. The file extension is added automatically."}
            </p>
            {formatError && (
              <p className="text-[12px] text-red-400">{formatError}</p>
            )}
          </div>
        )}
      </SettingsBlock>
    </div>
  );
};
