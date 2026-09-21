// Colour by meaning, as soft tints so that a status reads at a glance without shouting.
// Red is left to real problems: errors and destructive buttons.

// Text keeps its normal colour and a dot carries the meaning: tinted text at badge size was
// hard to read.
/** Waiting for something: a profile queued for the next login. */
export const WAITING = "text-amber-700 dark:text-amber-400";
/** Live right now: a connected account, a champion's settings that are on. */
export const DOT_LIVE = "bg-emerald-500";
export const DOT_OFF = "bg-muted-foreground/40";
