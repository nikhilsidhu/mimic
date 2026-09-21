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

/** Names League's own menus use, for the settings whose internal name says little. */
const NAMES: Record<string, string> = {
  CameraLockToggle: "Toggle camera lock",
  CameraSnap: "Center camera on champion",
  ChampionOnly: "Target champions only",
  DrawHud: "Toggle HUD",
  OpenShop: "Open shop",
  PlayerAttackMove: "Attack move",
  PlayerAttackMoveClick: "Attack move click",
  PlayerAttackOnlyClick: "Attack only click",
  PlayerHoldPosition: "Hold position",
  PlayerMoveClick: "Move click",
  PlayerStopPosition: "Stop",
  PlayerPingMIA: "Ping: enemy missing",
  PlayerPingOMW: "Ping: on my way",
  PushToTalk: "Push to talk",
  PushToTalkTeam: "Push to talk (team)",
  SelectSelf: "Select yourself",
  ShowScoreBoard: "Show scoreboard",
  HoldShowScoreBoard: "Show scoreboard (hold)",
  ShowFPSAndLatency: "Show FPS and ping",
  ToggleFPSAndLatency: "Toggle FPS and ping",
  SysMenu: "Open menu",
  UseVisionItem: "Trinket",
};

/** How a spell or item is cast, by the prefix of its internal name. Longest first. */
const CAST_MODES: [string, string][] = [
  ["SmartPlusSelfCastWithIndicator", "Quick + self cast {}, indicator"],
  ["SmartPlusSelfCast", "Quick + self cast {}"],
  ["SmartCastWithIndicator", "Quick cast {}, indicator"],
  ["SmartCast", "Quick cast {}"],
  ["SelfCast", "Self cast {}"],
  ["NormalCast", "Normal cast {}"],
  ["Cast", "Cast {}"],
  ["Use", "{}"],
];

const SPELL_KEYS = ["Q", "W", "E", "R"];

/** What is cast, the way players say it: `Spell4` is "R", `AvatarSpell1` a summoner spell. */
function castTarget(name: string): string | null {
  const [, kind, digit] = name.match(/^(Spell|AvatarSpell|Item)(\d)$/) ?? [];
  const number = Number(digit);
  if (kind === "Spell") return SPELL_KEYS[number - 1] ?? `spell ${number}`;
  if (kind === "AvatarSpell") return `summoner spell ${number}`;
  if (kind === "Item") return `item ${number}`;
  if (name === "VisionItem") return "trinket";
  if (name === "RoleBound") return "role item";
  return null;
}

const capitalised = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** `ShowFPSAndLatency` becomes "Show FPS and latency". */
function words(name: string): string {
  const spaced = name.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2");
  const lowered = spaced.split(" ").map((word) => (/^[A-Z0-9]+$/.test(word) ? word : word.toLowerCase()));
  const sentence = lowered.join(" ");
  return sentence.charAt(0).toUpperCase() + sentence.slice(1);
}

/** A setting's name as a person would say it: `evtSelfCastSpell2` is "Self cast W". */
export function settingLabel(key: string): string {
  const name = key.replace(/^evn?t/, "");
  if (NAMES[name]) return NAMES[name];

  // `CastSpell1smart` is the switch that turns quick cast on for that spell.
  const quick = name.endsWith("smart");
  const base = quick ? name.slice(0, -"smart".length) : name;
  for (const [prefix, mode] of CAST_MODES) {
    const target = base.startsWith(prefix) ? castTarget(base.slice(prefix.length)) : null;
    if (target) return capitalised(quick ? `Quick cast for ${target}` : mode.replace("{}", target));
  }

  const level = name.match(/^LevelSpell(\d)$/);
  if (level) return `Level up ${castTarget(`Spell${level[1]}`)}`;
  const ping = name.match(/^PlayerPing(.+)$/);
  if (ping) return `Ping: ${words(ping[1]).toLowerCase()}`;
  return words(name);
}
