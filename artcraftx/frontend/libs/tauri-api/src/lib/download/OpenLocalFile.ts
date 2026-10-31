import { invoke } from "@tauri-apps/api/core";
import { CommandResult } from "../common/CommandStatus";

export interface OpenLocalFileSuccess extends CommandResult {
  payload: Record<string, never>;
}

/** How `open_local_file_command` opens a file. */
export type OpenLocalFileMode = "open" | "reveal";

/**
 * Open a downloaded file with the OS default application (image viewer,
 * video player, ...). Rejects if the file no longer exists.
 */
export const OpenLocalFile = async (
  path: string,
): Promise<OpenLocalFileSuccess> => {
  return (await invoke("open_local_file_command", {
    path,
    mode: "open" satisfies OpenLocalFileMode,
  })) as OpenLocalFileSuccess;
};

/**
 * Open the file's directory in the OS file manager (Finder, Explorer, ...)
 * with the file highlighted. Rejects if the file no longer exists.
 */
export const RevealLocalFile = async (
  path: string,
): Promise<OpenLocalFileSuccess> => {
  return (await invoke("open_local_file_command", {
    path,
    mode: "reveal" satisfies OpenLocalFileMode,
  })) as OpenLocalFileSuccess;
};

/** The platform's name for its file manager, for button labels. */
export const getFileManagerName = (): string => {
  const ua = typeof navigator !== "undefined" ? navigator.userAgent : "";
  if (/Mac/i.test(ua)) return "Finder";
  if (/Windows/i.test(ua)) return "Explorer";
  return "file manager";
};

/** Last path component, for display. Handles both `/` and `\` separators. */
export const localFileBasename = (path: string): string => {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
};
