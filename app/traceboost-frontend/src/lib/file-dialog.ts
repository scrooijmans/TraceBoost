import { isTauriEnvironment } from "./bridge";

function normalizeDialogPath(result: string | null): string | null {
  if (typeof result !== "string") {
    return null;
  }

  const normalized = result.trim();
  return normalized.length > 0 ? normalized : null;
}

/**
 * Opens a native file picker for SEG-Y files.
 * Returns the selected file path, or null if cancelled.
 */
export async function pickSegyFile(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    return normalizeDialogPath(prompt("Enter SEG-Y file path:"));
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

  return normalizeDialogPath(result);
}

/**
 * Opens a native folder/save picker for the runtime store output.
 * Returns the selected path, or null if cancelled.
 */
export async function pickOutputFolder(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    return normalizeDialogPath(prompt("Enter output store path:"));
  }

  const { save } = await import("@tauri-apps/plugin-dialog");
  const result = await save({
    title: "Set Runtime Store Output Path",
    defaultPath: "survey.tbvol",
    filters: [
      { name: "Runtime Store", extensions: ["tbvol"] },
      { name: "All Files", extensions: ["*"] }
    ]
  });

  return normalizeDialogPath(result);
}

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
