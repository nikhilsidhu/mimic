/** Where a component reports what an action had to say: a sentence, and whether it failed. */
export type Report = (text: string, failed: boolean) => void;

/**
 * Runs an action and reports its outcome: the sentence it resolves to, if any, or the
 * reason it failed. Resolves to whether it worked, and never rejects.
 */
export async function attempt(report: Report, action: () => Promise<string | null | void>): Promise<boolean> {
  try {
    const said = await action();
    if (said) report(said, false);
    return true;
  } catch (err) {
    report(String(err), true);
    return false;
  }
}
