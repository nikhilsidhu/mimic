import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type ProfileView = {
  id: string;
  name: string;
  active: boolean;
  settings: number;
};

export type Champion = {
  id: number;
  name: string;
  /** A URL the web view can load the cached icon from. */
  icon: string;
  /** The logged-in account's mastery points on it; 0 if never played. */
  mastery: number;
};

/** A champion's own settings: what it overrides, as [key, value] pairs. */
export type Overlay = {
  champion: Champion;
  settings: [string, string][];
};

/** Everything the tray panel and the manager display. */
export type View = {
  /** The account's Riot ID, or why there is none. */
  status: string;
  connected: boolean;
  /** The connected account's profile icon (a loadable URL) and level. */
  account: { icon: string; level: number } | null;
  phase: string | null;
  /** What the account is up to, for a badge, e.g. "Swiftplay · In game". */
  activity: string | null;
  /** Whether this account applies its profile by itself at login. */
  autoApply: boolean;
  /** The profile waiting for the next login, by name. */
  pending: string | null;
  profiles: ProfileView[];
  /** The champion whose settings are on top of this account's base right now. */
  activeOverlay: Champion | null;
  overlays: Overlay[];
};

/** One setting that differs. `from` or `to` is null when the key exists on one side only. */
export type Change = {
  file: string;
  section: string;
  key: string;
  from: string | null;
  to: string | null;
};

/** Settings the user changed, and where they could be saved. */
export type Drift = {
  profile: string | null;
  /** The champion they were made on, if known. */
  champion: { id: number; name: string } | null;
  changes: Change[];
  /** The changes are Riot's doing: the account's settings were reset, as after a patch. */
  reset: boolean;
};

/** Somewhere a champion's settings could be taken from. `profile` is null for what changed just now. */
export type OverlaySource = {
  profile: string | null;
  name: string;
  /** How many settings it would override. */
  settings: number;
};

export type DriftChoice = "saveToProfile" | "saveToChampion" | "revert" | "keepHere";

// The backend hands out file paths; the web view needs asset-protocol URLs.
const withIconUrl = (champion: Champion): Champion => ({ ...champion, icon: convertFileSrc(champion.icon) });

export async function getView(): Promise<View> {
  const view = await invoke<View>("view");
  return {
    ...view,
    account: view.account && { ...view.account, icon: convertFileSrc(view.account.icon) },
    activeOverlay: view.activeOverlay && withIconUrl(view.activeOverlay),
    overlays: view.overlays.map((overlay) => ({ ...overlay, champion: withIconUrl(overlay.champion) })),
  };
}

/** Every champion, by name. Empty until a League client has been seen once. */
export async function getChampions(): Promise<Champion[]> {
  return (await invoke<Champion[]>("champions")).map(withIconUrl);
}

export const getOverlaySources = () => invoke<OverlaySource[]>("overlay_sources");
export const getDrift = () => invoke<Drift | null>("drift");

function on(event: string, onEvent: () => void): () => void {
  const unlisten = listen(event, onEvent);
  return () => void unlisten.then((stop) => stop());
}

/** Calls `onChange` whenever the view may have changed. Returns a function that stops listening. */
export const onViewChanged = (onChange: () => void) => on("view-changed", onChange);
/** Calls `onChange` when the changed-settings prompt should read the drift again. */
export const onDriftChanged = (onChange: () => void) => on("drift-changed", onChange);

// Every action resolves to a sentence to show, and rejects with one on failure.
export const applyProfile = (id: string) => invoke<string>("apply_profile", { id });
export const renameProfile = (id: string, name: string) => invoke<string>("rename_profile", { id, name });
export const deleteProfile = (id: string) => invoke<string>("delete_profile", { id });
export const saveOverlay = (champion: number, profile: string | null) =>
  invoke<string>("save_overlay", { champion, profile });
export const deleteOverlay = (champion: number) => invoke<string>("delete_overlay", { champion });
export const saveCurrent = (name: string) => invoke<string>("save_current", { name });
export const undoLast = () => invoke<string>("undo_last");
export const resolveDrift = (choice: DriftChoice) => invoke<string>("resolve_drift", { choice });
export const copyRiotId = () => invoke<string>("copy_riot_id");

export const setAutoApply = (enabled: boolean) => invoke<void>("set_auto_apply", { enabled });
export const openManager = () => invoke<void>("open_manager");
export const openLogs = () => invoke<void>("open_logs");
/** Shows a sentence in the notice popup next to the tray. */
export const notify = (message: string) => invoke<void>("notify", { message });
/** Closes the tray panel. */
export const dismiss = () => invoke<void>("dismiss");
export const quit = () => invoke<void>("quit");
