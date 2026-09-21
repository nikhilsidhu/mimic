// The colour theme, shared by every window through localStorage. `app.html` applies it
// before the first paint; this keeps it current afterwards.

export type Theme = "system" | "light" | "dark";

const KEY = "theme";
const prefersDark = () => window.matchMedia("(prefers-color-scheme: dark)");

export function getTheme(): Theme {
  const stored = localStorage.getItem(KEY);
  return stored === "light" || stored === "dark" ? stored : "system";
}

function apply() {
  const theme = getTheme();
  const dark = theme === "dark" || (theme === "system" && prefersDark().matches);
  document.documentElement.classList.toggle("dark", dark);
}

export function setTheme(theme: Theme) {
  if (theme === "system") localStorage.removeItem(KEY);
  else localStorage.setItem(KEY, theme);
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
