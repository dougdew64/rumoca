/**
 * What actually happened to a breakpoint-arm request — the payload of the ack.
 *
 * ## Why this file exists, and why it imports no `vscode`
 *
 * The ack used to be `{"acked": true}`, written unconditionally at the end of
 * every request. It answered **"I read your file"** while HRW read it as
 * **"a breakpoint exists"** — `live_breakpoint_armed = acked`. Those are
 * different claims, and `docs/ideas.md` #71 is the entry about HRW asserting
 * state it cannot see.
 *
 * The gap became reachable rather than theoretical on 2026-08-08. `#74` made
 * *skipping* the normal path: after the first Debug press of a session the
 * anchor is already armed, so every later press correctly adds nothing. And
 * `isDuplicate` did not check whether the breakpoint it found was **enabled**,
 * so one click of VS Code's *Disable All Breakpoints* produced: nothing armed,
 * nothing enabled, ack `true`, HRW claiming armed, and the algorithm running to
 * completion with no stop and no notice.
 *
 * So the ack now answers one precise question:
 *
 * > **Does an ENABLED breakpoint now exist at every requested line?**
 *
 * Not "did I add one" — an already-present enabled breakpoint is a perfectly
 * good yes, and is what a hand-set anchor or a repeat Debug press produces.
 *
 * ## What this still cannot promise, and the limit is VS Code's
 *
 * **"Enabled and present" is not "bound".** A breakpoint the adapter has
 * declined to verify sits in `vscode.debug.breakpoints` looking exactly like a
 * working one — VS Code draws it hollow, but exposes **no `verified` field to
 * extensions**, so nothing here can tell the difference. `#74` is the case that
 * matters: after a location's breakpoint has been removed or disabled once,
 * `cppvsdbg` will not re-bind it for the rest of the session.
 *
 * So this verdict is a large improvement on "I read your file" and is still not
 * proof that execution will stop. **Do not let a future reader mistake
 * `breakpointPresent` for that**, and do not paper over it with a guess.
 *
 * Imports nothing from `vscode` so `node --test` can exercise it directly; the
 * caller maps `vscode.debug.breakpoints` into [`ExistingBreakpoint`] records
 * first. This is the same move `debug_state.ts` makes, for the same reason:
 * `extension.ts` cannot be tested at all.
 */

/** A breakpoint request entry, as HRW writes it. Lines are 1-based. */
export interface RequestedBreakpoint {
    path: string;
    line: number;
    condition?: string | null;
}

/**
 * A breakpoint VS Code already knows about, flattened to plain data.
 *
 * `line` is **1-based here**, converted by the caller, so this module never has
 * to remember which side of the boundary it is on.
 */
export interface ExistingBreakpoint {
    path: string;
    line: number;
    condition?: string | null;
    enabled: boolean;
}

/** What became of one requested line. */
export type EntryOutcome =
    /** Newly added by this request. */
    | 'armed'
    /** An enabled breakpoint was already there — nothing to do, and a real yes. */
    | 'alreadyEnabled'
    /** A breakpoint is there but is DISABLED, so nothing will stop. */
    | 'disabled'
    /** The source file does not exist, so no breakpoint could be created. */
    | 'fileMissing';

export interface EntryVerdict {
    path: string;
    line: number;
    outcome: EntryOutcome;
}

export interface ArmVerdict {
    /** Ack schema version. HRW refuses to guess when this is absent. */
    version: 2;
    acked: true;
    action: 'add' | 'remove';
    entries: EntryVerdict[];
    requested: number;
    armed: number;
    alreadyEnabled: number;
    /**
     * **The field HRW acts on.** True only when every requested line now has an
     * enabled breakpoint. A `remove` request reports `false` — nothing is meant
     * to be in place afterwards — and HRW does not poll after a remove.
     */
    breakpointPresent: boolean;
    /** Why not, in one sentence, when `breakpointPresent` is false. */
    reason?: string;
}

/** Case-insensitive path comparison — Windows paths differ only in case. */
function samePath(a: string, b: string): boolean {
    return a.toLowerCase() === b.toLowerCase();
}

function sameCondition(
    a: string | null | undefined,
    b: string | null | undefined,
): boolean {
    return (a ?? undefined) === (b ?? undefined);
}

/**
 * Find an existing breakpoint covering `entry`, enabled or not.
 *
 * **Searches ALL breakpoints, not only ones the extension armed** (`1585432d`).
 * The anchor is a documented breakpoint site, so the user may well have set one
 * by hand; adding a second at the same location leaves two indistinguishable
 * entries in the Breakpoints list. Skipping is also the safer behaviour on the
 * way out, since `handleRemove` only ever removes what the extension added — so
 * a hand-set breakpoint survives the end of the live session, which is what the
 * user meant by setting it.
 *
 * **The condition is part of the match**: a conditional request at a line that
 * already has an unconditional breakpoint is genuinely a different breakpoint
 * and must not be treated as covered.
 *
 * **Returns the breakpoint rather than a boolean** so the caller can read
 * `enabled`. Its predecessor `isDuplicate` returned a bool, which is precisely
 * why a disabled breakpoint could pass for a working one.
 */
export function findExisting(
    entry: RequestedBreakpoint,
    existing: ExistingBreakpoint[],
): ExistingBreakpoint | undefined {
    return existing.find(
        bp =>
            samePath(bp.path, entry.path) &&
            bp.line === entry.line &&
            sameCondition(bp.condition, entry.condition),
    );
}

/**
 * Decide, per entry, what an add request should do and what it achieved.
 *
 * `fileExists` is injected rather than called, so the tests need no fixtures on
 * disk — and so this module keeps its "no I/O, no vscode" property.
 */
export function planAdd(
    entries: RequestedBreakpoint[],
    existing: ExistingBreakpoint[],
    fileExists: (path: string) => boolean,
): EntryVerdict[] {
    return entries.map(entry => {
        if (!fileExists(entry.path)) {
            return { path: entry.path, line: entry.line, outcome: 'fileMissing' as const };
        }
        const found = findExisting(entry, existing);
        if (!found) {
            return { path: entry.path, line: entry.line, outcome: 'armed' as const };
        }
        return {
            path: entry.path,
            line: entry.line,
            outcome: (found.enabled ? 'alreadyEnabled' : 'disabled') as EntryOutcome,
        };
    });
}

/** Short human phrase for an outcome that means "nothing will stop here". */
function describeFailure(v: EntryVerdict): string {
    const where = `${v.path}:${v.line}`;
    switch (v.outcome) {
        case 'disabled':
            // **Re-enabling is NOT the remedy, and saying so was wrong** (found
            // by walking it, 2026-08-08). `cppvsdbg` does not re-bind a location
            // whose breakpoint has left its active set — `#74` proved that for
            // remove-then-add, and disable-then-enable is the same one-way door:
            // the marker stays hollow and nothing stops. Only a new debug
            // session recovers it.
            return `a breakpoint exists at ${where} but is DISABLED — re-enabling it will NOT restore it in this session (cppvsdbg does not re-bind a released location). Stop the debugger and start a new session.`;
        case 'fileMissing':
            return `no such file: ${where}`;
        default:
            return `${where} was not armed`;
    }
}

/**
 * Summarize per-entry outcomes into the ack payload.
 *
 * **An empty request reports `breakpointPresent: false`.** "Every one of zero
 * lines is armed" is vacuously true and exactly the kind of true-but-useless
 * answer this whole change exists to remove.
 */
export function summarize(
    action: 'add' | 'remove',
    entries: EntryVerdict[],
): ArmVerdict {
    const armed = entries.filter(e => e.outcome === 'armed').length;
    const alreadyEnabled = entries.filter(e => e.outcome === 'alreadyEnabled').length;
    const failures = entries.filter(
        e => e.outcome !== 'armed' && e.outcome !== 'alreadyEnabled',
    );

    const present =
        action === 'add' && entries.length > 0 && failures.length === 0;

    const verdict: ArmVerdict = {
        version: 2,
        acked: true,
        action,
        entries,
        requested: entries.length,
        armed,
        alreadyEnabled,
        breakpointPresent: present,
    };

    if (!present) {
        if (action === 'remove') {
            verdict.reason = 'this was a removal request; nothing is meant to be armed';
        } else if (entries.length === 0) {
            verdict.reason = 'the request named no breakpoints';
        } else {
            verdict.reason = failures.map(describeFailure).join('; ');
        }
    }

    return verdict;
}

/** One line for the output channel, so the log says what the ack says. */
export function describeVerdict(v: ArmVerdict): string {
    if (v.breakpointPresent) {
        return `Ack: ${v.requested} line(s) covered — ${v.armed} newly armed, ${v.alreadyEnabled} already enabled`;
    }
    return `Ack: NOT armed — ${v.reason ?? 'no reason given'}`;
}
