
import { download } from "@tauri-apps/plugin-upload";
import { downloadDir } from "@tauri-apps/api/path";
import { open } from "@tauri-apps/plugin-dialog";

const deriveDownloadFilename = (url: string): string => {
  try {
    const urlObj = new URL(url);
    const last = urlObj.pathname.split("/").pop();
    if (last && last.length > 0) return last;
  } catch {
    // fall through
  }
  return "downloaded_file";
};

/**
 * Prompts the user to pick a directory (e.g. for batch downloads).
 * Returns the chosen absolute path, or `null` when dismissed.
 */
export const pickDownloadDirectory = async (): Promise<string | null> => {
  const chosen = await open({ directory: true, multiple: false });
  return typeof chosen === "string" ? chosen : null;
};

/** Downloads `url` to an explicit absolute filesystem path. */
export const downloadUrlToPath = async (url: string, path: string) => {
  await download(url, path);
};

export const downloadFileFromUrl = async (url: string) => {
  console.log("GOT THE URL", url);
  try {
    const filename = deriveDownloadFilename(url);

    const downloadsPath = await downloadDir();
    const filePath = `${downloadsPath}/${filename}`;

    await download(url, filePath);

    console.log(
      `File downloaded and saved to ${filePath}`,
    );
  } catch (error) {
    console.error("Error downloading file:", error);
    throw error;
  }
};
