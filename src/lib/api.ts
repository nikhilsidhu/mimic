import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ProfileView = {
  id: string;
  name: string;
  active: boolean;
  settings: number;
};

/** Everything the tray panel displays. */
export type View = {
  /** The account's Riot ID, or why there is none. */
  status: string;
  connected: boolean;
  phase: string | null;
  /** Whether this account applies its profile by itself at login. */
  autoApply: boolean;
  /** The profile waiting for the next login, by name. */
  pending: string | null;
  profiles: ProfileView[];
};

/** One setting that differs. `from` or `to` is null when the key exists on one side only. */
export type Change = {
  file: string;
  section: string;
  key: string;
  from: string | null;
  to: string | null;
};

/** Settings the user changed, and the name of the profile they could be saved to. */
export type Drift = {
  profile: string | null;
  changes: Change[];
};

export type DriftChoice = "saveToProfile" | "revert" | "keepHere";

export type Champion = {
  id: number;
  name: string;
  /** A URL the web view can load the cached icon from. */
  icon: string;
};

/** Every champion, by name. Empty until a League client has been seen once. */
export async function getChampions(): Promise<Champion[]> {
  const champions = await invoke<Champion[]>("champions");
  return champions.map((champion) => ({ ...champion, icon: convertFileSrc(champion.icon) }));
}

export const getView = () => invoke<View>("view");
export const getDrift = () => invoke<Drift | null>("drift");
export const resolveDrift = (choice: DriftChoice) => invoke<string>("resolve_drift", { choice });

/** Calls `onChange` when the changed-settings prompt should read the drift again. */
export function onDriftChanged(onChange: () => void): () => void {
  const unlisten = listen("drift-changed", onChange);
  return () => void unlisten.then((stop) => stop());
}

/** Calls `onChange` whenever the view may have changed. Returns a function that stops listening. */
export function onViewChanged(onChange: () => void): () => void {
  const unlisten = listen("view-changed", onChange);
  return () => void unlisten.then((stop) => stop());
}

// Every action resolves to a sentence to show, and rejects with one on failure.
export const applyProfile = (id: string) => invoke<string>("apply_profile", { id });
export const renameProfile = (id: string, name: string) => invoke<string>("rename_profile", { id, name });
export const deleteProfile = (id: string) => invoke<string>("delete_profile", { id });
export const setAutoApply =(enabled: boolean) => invoke<void>("set_auto_apply", { enabled });
export const saveCurrent =(name: string) => invoke<string>("save_current", { name });
export const undoLast = () => invoke<string>("undo_last");
export const copyRiotId = () => invoke<string>("copy_riot_id");
export const openManager = () => invoke<void>("open_manager");
export const openLogs = () => invoke<void>("open_logs");
/** Shows a sentence in the notice popup next to the tray. */
export const notify = (message: string) => invoke<void>("notify", { message });
/** Closes the tray flyout (panel or menu) this page runs in. */
export const dismiss = () => invoke<void>("dismiss");
export const quit = () => invoke<void>("quit");
