import { invoke } from "@tauri-apps/api/core";
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

export const getView = () => invoke<View>("view");

/** Calls `onChange` whenever the view may have changed. Returns a function that stops listening. */
export function onViewChanged(onChange: () => void): () => void {
  const unlisten = listen("view-changed", onChange);
  return () => void unlisten.then((stop) => stop());
}

// Every action resolves to a sentence to show, and rejects with one on failure.
export const applyProfile = (id: string) => invoke<string>("apply_profile", { id });
export const setAutoApply = (enabled: boolean) => invoke<void>("set_auto_apply", { enabled });
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
