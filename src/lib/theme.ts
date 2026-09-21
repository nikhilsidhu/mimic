// The colour theme, shared by every window. It is kept in localStorage, which `app.html`
// reads before the first paint, and a change is broadcast as a Tauri event: the browser's own
// `storage` event does not reliably reach the other windows, least of all hidden ones.

import { emit, listen } from "@tauri-apps/api/event";

export type Theme = "system" | "light" | "dark" | "black";

const KEY = "theme";
const CHANGED = "theme-changed";
/** Every theme, in the order they are offered. */
export const THEMES: Theme[] = ["system", "light", "dark", "black"];
const prefersDark = () => window.matchMedia("(prefers-color-scheme: dark)");

/** Black unless something else was chosen: it is how mimic is meant to look. */
export function getTheme(): Theme {
  const stored = localStorage.getItem(KEY) as Theme | null;
  return stored && THEMES.includes(stored) ? stored : "black";
}

function apply() {
  const theme = getTheme();
  const dark = theme === "dark" || theme === "black" || (theme === "system" && prefersDark().matches);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.classList.toggle("black", theme === "black");
}

export function setTheme(theme: Theme) {
  localStorage.setItem(KEY, theme);
  apply();
  void emit(CHANGED);
}

/** Follows changes made in another window or in Windows itself. Returns a function that stops. */
export function watchTheme(): () => void {
  apply();
  const system = prefersDark();
  const unlisten = listen(CHANGED, apply);
  system.addEventListener("change", apply);
  // A window that was hidden while the theme changed catches up when it is shown again.
  document.addEventListener("visibilitychange", apply);
  return () => {
    void unlisten.then((stop) => stop());
    system.removeEventListener("change", apply);
    document.removeEventListener("visibilitychange", apply);
  };
}
