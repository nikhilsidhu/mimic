// Colour by meaning. The colours themselves are tokens in `app.css`, `--live` and `--waiting`,
// so that each is changed in one place; red is left to real problems.
//
// Text keeps its normal colour and a dot carries the meaning: tinted text at badge size was
// hard to read.

/** Live right now: a connected account, a champion's settings that are on. */
export const DOT_LIVE = "bg-live";
/** Awaiting something: a profile that was edited since it was applied. */
export const DOT_WAITING = "bg-waiting";
export const DOT_OFF = "bg-muted-foreground/40";
/** Text that says something is waiting, such as a profile queued for the next login. */
export const WAITING = "text-waiting";
