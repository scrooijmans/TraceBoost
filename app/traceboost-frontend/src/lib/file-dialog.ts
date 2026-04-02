import { isTauriEnvironment } from "./bridge";

function normalizeDialogPath(result: string | null): string | null {
  if (typeof result !== "string") {
    return null;
  }

  const normalized = result.trim();
  return normalized.length > 0 ? normalized : null;
}

/**
 * Opens a native file picker for TraceBoost-supported seismic volumes.
 * Returns the selected file path, or null if cancelled.
 */
export async function pickVolumeFile(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    return normalizeDialogPath(prompt("Enter volume path (.segy, .sgy, .tbvol):"));
  }

  const { open } = await import("@tauri-apps/plugin-dialog");
  const result = await open({
    title: "Open Volume",
    filters: [
      { name: "Supported Volumes", extensions: ["tbvol", "sgy", "segy"] },
      { name: "Runtime Stores", extensions: ["tbvol"] },
      { name: "SEG-Y Files", extensions: ["sgy", "segy"] },
      { name: "All Files", extensions: ["*"] }
    ],
    multiple: false,
    directory: false
  });

  return normalizeDialogPath(result);
}

export const pickSegyFile = pickVolumeFile;

/**
 * Opens a native folder/save picker for the runtime store output.
 * Returns the selected path, or null if cancelled.
 */
export async function pickOutputStorePath(defaultPath = "survey.tbvol"): Promise<string | null> {
  if (!isTauriEnvironment()) {
    return normalizeDialogPath(prompt("Enter output store path:"));
  }

  const { save } = await import("@tauri-apps/plugin-dialog");
  const result = await save({
    title: "Set Runtime Store Output Path",
    defaultPath,
    filters: [
      { name: "Runtime Store", extensions: ["tbvol"] },
      { name: "All Files", extensions: ["*"] }
    ]
  });

  return normalizeDialogPath(result);
}

export const pickOutputFolder = pickOutputStorePath;

export async function confirmOverwriteStore(outputStorePath: string): Promise<boolean> {
  const message = [
    "A runtime store already exists at this location.",
    "",
    outputStorePath,
    "",
    "Overwrite it and replace the existing .tbvol store?"
  ].join("\n");

  if (!isTauriEnvironment()) {
    return window.confirm(message);
  }

  const { confirm } = await import("@tauri-apps/plugin-dialog");
  return confirm(message, {
    title: "Overwrite Existing Runtime Store?",
    kind: "warning",
    okLabel: "Overwrite",
    cancelLabel: "Cancel"
  });
}
