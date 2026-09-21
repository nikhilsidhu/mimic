// The colour theme, shared by every window through localStorage. `app.html` applies it
// before the first paint; this keeps it current afterwards.

export type Theme = "system" | "light" | "dark" | "black";

const KEY = "theme";
/** Every theme, in the order they are offered. */
export const THEMES: Theme[] = ["system", "light", "dark", "black"];
const prefersDark = () => window.matchMedia("(prefers-color-scheme: dark)");

/** Dark unless something else was chosen: it is how mimic is meant to look. */
export function getTheme(): Theme {
  const stored = localStorage.getItem(KEY) as Theme | null;
  return stored && THEMES.includes(stored) ? stored : "dark";
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
}

/** Follows changes made in another window or in Windows itself. Returns a function that stops. */
export function watchTheme(): () => void {
  apply();
  const system = prefersDark();
  // `storage` only fires in the windows that did not make the change.
  window.addEventListener("storage", apply);
  system.addEventListener("change", apply);
  return () => {
    window.removeEventListener("storage", apply);
    system.removeEventListener("change", apply);
  };
}
