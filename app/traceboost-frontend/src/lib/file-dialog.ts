import { isTauriEnvironment } from "./bridge";

/**
 * Opens a native file picker for SEG-Y files.
 * Returns the selected file path, or null if cancelled.
 */
export async function pickSegyFile(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    return prompt("Enter SEG-Y file path:");
  }

  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({
    title: "Select SEG-Y File",
    filters: [
      { name: "SEG-Y Files", extensions: ["sgy", "segy"] },
      { name: "All Files", extensions: ["*"] }
    ],
    multiple: false,
    directory: false
  });

  return result ?? null;
}

/**
 * Opens a native folder/save picker for the runtime store output.
 * Returns the selected path, or null if cancelled.
 */
export async function pickOutputFolder(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    return prompt("Enter output store path:");
  }

  const { save } = await import("@tauri-apps/plugin-dialog");
  const result = await save({
    title: "Set Runtime Store Output Path",
    defaultPath: "survey.zarr",
    filters: [
      { name: "Zarr Store", extensions: ["zarr"] },
      { name: "All Files", extensions: ["*"] }
    ]
  });

  return result ?? null;
}
