// League writes a bind as "[Shift][q]", or as two alternatives: "[<Unbound>],[x]".

const LABELS: Record<string, string> = {
  "Button 1": "LMB",
  "Button 2": "RMB",
  "Button 3": "MMB",
  "Button 4": "M4",
  "Button 5": "M5",
  Return: "Enter",
  Back: "Bksp",
  Escape: "Esc",
  "Caps Lock": "Caps",
};

/** One way to press a bind: the keys held together, e.g. ["Shift", "Q"]. */
export type Chord = string[];

/** The alternatives of a bind, unbound ones left out. Empty means nothing is bound. */
export function parseBind(value: string | null): Chord[] {
  return (value ?? "")
    .split(",")
    .map((alternative) =>
      [...alternative.matchAll(/\[([^\]]+)\]/g)]
        .map(([, key]) => key)
        .filter((key) => key !== "<Unbound>")
        .map((key) => LABELS[key] ?? (key.length === 1 ? key.toUpperCase() : key)),
    )
    .filter((chord) => chord.length > 0);
}

/** Whether a changed setting is a keybind rather than a plain value. */
export const isBind = (change: { file: string; section: string }) =>
  change.file === "Input.ini" && change.section !== "Quickbinds";

/** `evtCastSpell1` reads better as `CastSpell1`. */
export const settingLabel = (key: string) => key.replace(/^evn?t/, "");
