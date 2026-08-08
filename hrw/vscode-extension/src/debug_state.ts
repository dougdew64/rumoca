/**
 * Assembly of the debug-state payload Claude reads — `docs/ideas.md` #72.
 *
 * ## Why this file has no `vscode` import
 *
 * Everything here is a pure function over plain data, so `node --test` can
 * exercise it directly. The `vscode` module only exists inside the extension
 * host, so anything importing it is untestable outside VS Code — and the
 * existing `extension_surface.test.mjs` shows what that forces you into: tests
 * that build an object and assert its own fields. This is the same move HRW
 * makes on the Rust side (`Plot::problems()`), for the same reason: push the
 * logic out of the untestable layer and leave a thin wiring shell behind.
 *
 * ## The contract this file exists to enforce
 *
 * Claude answers questions about a running algorithm from this payload. So the
 * rule from `hrw/CLAUDE.md` applies at full strength: **nothing may be
 * invented, and absence must be stated rather than filled.** Two distinctions
 * carry that, and both are tested:
 *
 * - `variables: []` means *"fetched, and there were none"*.
 *   `variables: null` means *"not fetched"* — with `variablesError` saying why.
 *   Collapsing the second into the first would have Claude report "no locals"
 *   about a frame it never managed to read.
 * - Truncation is **declared**. A capped list carries `framesTruncated` /
 *   `variablesTruncated` plus the true count, because a silently shortened list
 *   reads as a complete one. (`hrw/CLAUDE.md`, "no silent caps".)
 *
 * And staleness is the third: a payload from the *previous* stop is worse than
 * no payload, because Claude would describe the wrong state confidently. Every
 * write carries `seq` and `writtenAtMs`, and [`isStale`] is what a reader uses
 * before trusting it.
 */

/** File Claude reads, inside `.hrw-bridge/`. */
export const DEBUG_STATE_FILE = 'debug-state.json';

/** Payload version. Bump when a field's meaning changes, never for additions. */
export const DEBUG_STATE_VERSION = 1;

/** Default caps. Generous — a truncated payload is honest but less useful. */
export const DEFAULT_FRAME_LIMIT = 40;
export const DEFAULT_VARIABLE_LIMIT = 60;

/**
 * Elements kept when expanding one aggregate.
 *
 * Sized for the systems under study rather than for MSL: `Drivetrain` has 48
 * equations, and a 64-element window shows a whole `match_eq` for anything a
 * tour uses. A bigger model truncates, and says so via `childrenTruncated`.
 */
export const CHILD_LIMIT = 64;

/** A stack frame as the Debug Adapter Protocol reports it, already flattened. */
export interface RawFrame {
    name: string;
    path?: string;
    line?: number;
    column?: number;
}

/** One local, as DAP reports it. `value` is the adapter's own rendering. */
export interface RawVariable {
    name: string;
    value: string;
    type?: string;
    /**
     * DAP's expansion handle. Greater than zero means the value shown is a
     * *summary* — `cppvsdbg` renders a slice as `{ len=2 }` — and the elements
     * live behind another `variables` request.
     */
    variablesReference?: number;
    /** One level of expansion, when it was fetched. Absent means not fetched. */
    children?: RawVariable[];
    childrenTruncated?: boolean;
}

/**
 * A local as published, with the adapter's non-answers marked.
 *
 * **`available: false` is the point of this type.** `cppvsdbg` reports a local
 * that is not live at the current program point as the *string*
 * `"Variable is optimized away and not available."`, which is prose in a field
 * Claude reads as data. Measured 2026-08-08: stopped at `augment_traced`'s
 * `for var in vars` loop head, four of twelve locals came back that way — `var`
 * is not bound yet, `holder` is in an unreached arm, `can_augment` is assigned
 * later, `iter` is the desugared iterator. **All four are honest absences at
 * that line**, and none of them is a value.
 *
 * Passing the prose through unmarked let `variableCount: 12` overstate what was
 * actually known by four.
 */
export interface PublishedVariable extends RawVariable {
    available: boolean;
    children?: PublishedVariable[];
}

/**
 * Adapter renderings that mean "no value here", not a value.
 *
 * Deliberately narrow. A broad pattern risks discarding a real value whose text
 * happens to contain one of these words, and a false `available: false` hides
 * data — the opposite failure, equally bad.
 */
const UNAVAILABLE_PATTERNS: readonly RegExp[] = [
    /optimized away/i,
    /^<optimized out>$/i,
    /^<not available>$/i,
    /cannot be evaluated/i,
];

/** True when the adapter's rendering is a statement of absence. */
export function isUnavailableValue(value: string | undefined): boolean {
    if (value === undefined || value === '') {
        return true;
    }
    return UNAVAILABLE_PATTERNS.some((re) => re.test(value));
}

/** Mark a variable and, recursively, whatever was expanded beneath it. */
function markAvailability(v: RawVariable): PublishedVariable {
    return {
        ...v,
        available: !isUnavailableValue(v.value),
        children: v.children?.map(markAvailability),
    };
}

/** Where execution is stopped, derived from the innermost frame. */
export interface StopLocation {
    path?: string;
    line?: number;
    frame: string;
}

export interface DebugStateInput {
    /** Monotonic per session-run. The caller owns incrementing it. */
    seq: number;
    /** `Date.now()` at write time. */
    writtenAtMs: number;
    sessionId?: string;
    sessionName?: string;
    /** False while running, or once the session ends. */
    stopped: boolean;
    /** DAP stop reason: `breakpoint`, `step`, `exception`, … */
    reason?: string;
    threadId?: number;
    /** Innermost first, as DAP returns them. */
    frames?: RawFrame[];
    /**
     * Locals of the innermost frame. **`null` means not fetched** — pass
     * `variablesError` alongside. `[]` means genuinely none.
     */
    variables?: RawVariable[] | null;
    variablesError?: string;
    /** Which DAP scope the variables came from, e.g. `Locals`. */
    variablesScope?: string;
    /**
     * What was asked of `stackTrace`, and what each shape returned — e.g.
     * `["levels=40 -> 0", "threadId only -> 7"]`.
     *
     * **This exists because the first attempt at #72 got zero frames and could
     * not say why.** `cppvsdbg` returned nothing for `levels: 0`, which DAP
     * defines as "all frames" — so "the adapter reported no stack frames" was
     * true and useless, indistinguishable from a thread that genuinely had
     * none. Publishing the attempts turns the next run into a measurement
     * instead of another guess.
     */
    stackAttempts?: string[];
    /** The request shape that actually produced frames, if any did. */
    stackShape?: string;
    frameLimit?: number;
    variableLimit?: number;
}

export interface DebugState {
    version: number;
    seq: number;
    writtenAtMs: number;
    writtenAtIso: string;
    sessionId: string | null;
    sessionName: string | null;
    stopped: boolean;
    reason: string | null;
    threadId: number | null;
    /** Null whenever not stopped, or when no frame was reported. */
    location: StopLocation | null;
    /** Possibly capped; `frameCount` is always the true total. */
    frames: RawFrame[];
    frameCount: number;
    framesTruncated: boolean;
    variables: PublishedVariable[] | null;
    variableCount: number | null;
    /**
     * How many of the published locals carry no value. Null when none were
     * fetched. **Read this before trusting `variableCount`** — a frame can
     * report twelve locals of which four are absences.
     */
    variablesUnavailable: number | null;
    variablesTruncated: boolean;
    variablesError: string | null;
    variablesScope: string | null;
    /** Every `stackTrace` shape tried, with the frame count each returned. */
    stackAttempts: string[];
    /** The shape that produced frames, or null if none did. */
    stackShape: string | null;
}

/**
 * Build the payload. Total function — every field is always present, so a
 * reader never has to distinguish "absent key" from "no data".
 */
export function buildDebugState(input: DebugStateInput): DebugState {
    const frameLimit = input.frameLimit ?? DEFAULT_FRAME_LIMIT;
    const variableLimit = input.variableLimit ?? DEFAULT_VARIABLE_LIMIT;

    // **Not stopped means no location, whatever else was passed.** A caller
    // reporting `continued` has no business publishing a frame, and letting one
    // through would leave Claude describing a position the program has left.
    const allFrames = input.stopped ? (input.frames ?? []) : [];
    const frames = allFrames.slice(0, frameLimit);

    const innermost = frames[0];
    const location: StopLocation | null = innermost
        ? { path: innermost.path, line: innermost.line, frame: innermost.name }
        : null;

    // `null` survives as `null`. Only a real array is capped or counted.
    const allVariables = input.stopped ? (input.variables ?? null) : null;
    const variables = allVariables === null
        ? null
        : allVariables.slice(0, variableLimit).map(markAvailability);

    return {
        version: DEBUG_STATE_VERSION,
        seq: input.seq,
        writtenAtMs: input.writtenAtMs,
        writtenAtIso: new Date(input.writtenAtMs).toISOString(),
        sessionId: input.sessionId ?? null,
        sessionName: input.sessionName ?? null,
        stopped: input.stopped,
        reason: input.stopped ? (input.reason ?? null) : null,
        threadId: input.stopped ? (input.threadId ?? null) : null,
        location,
        frames,
        frameCount: allFrames.length,
        framesTruncated: allFrames.length > frames.length,
        variables,
        variableCount: allVariables === null ? null : allVariables.length,
        // Counted over what was PUBLISHED, not over `allVariables`: the cap may
        // have removed some, and a count covering rows nobody can see would be
        // its own small fiction.
        variablesUnavailable:
            variables === null ? null : variables.filter((v) => !v.available).length,
        variablesTruncated:
            allVariables !== null && variables !== null
                ? allVariables.length > variables.length
                : false,
        variablesError: input.stopped ? (input.variablesError ?? null) : null,
        variablesScope: input.stopped ? (input.variablesScope ?? null) : null,
        // Kept even when not stopped: a failed probe is worth reading after the
        // fact, and unlike a location it does not describe a position the
        // program has since left.
        stackAttempts: input.stackAttempts ?? [],
        stackShape: input.stackShape ?? null,
    };
}

/**
 * True when the payload is too old to trust.
 *
 * **A reader must call this before believing anything else.** The failure this
 * prevents is the expensive one: a leftover payload describing the *previous*
 * step, reported with the same confidence as a current one. There is no way to
 * tell those apart from the content, which is exactly why the timestamp is not
 * optional.
 */
export function isStale(
    state: Pick<DebugState, 'writtenAtMs'>,
    nowMs: number,
    maxAgeMs: number,
): boolean {
    return nowMs - state.writtenAtMs > maxAgeMs;
}

/**
 * One-line summary for the "HRW Bridge" output channel.
 *
 * Exists so the feature is *visibly* working: per `hrw/CLAUDE.md`'s must-fire
 * rule, a reporter that reports nothing must not look identical to one with
 * nothing to report.
 */
export function describeStop(state: DebugState): string {
    if (!state.stopped) {
        return `#${state.seq} running`;
    }
    const where = state.location
        ? `${basename(state.location.path)}:${state.location.line ?? '?'} in ${state.location.frame}`
        : 'no frame reported';
    // "12 var(s)" overstated a frame where four of them were absences, so the
    // dead ones are named in the same breath as the total.
    const dead = state.variablesUnavailable ?? 0;
    const vars = state.variables === null
        ? `vars unavailable (${state.variablesError ?? 'no reason given'})`
        : dead > 0
            ? `${state.variableCount} var(s), ${dead} not live here`
            : `${state.variableCount} var(s)`;
    // Naming the winning shape makes the adapter's behaviour visible in the
    // channel rather than only in the JSON — and when nothing worked, the
    // tally is the whole diagnostic, so it is printed instead of hidden.
    const depth = state.stackShape
        ? `${state.frameCount} frame(s) via ${state.stackShape}`
        : `${state.frameCount} frame(s) [${state.stackAttempts.join('; ')}]`;
    return `#${state.seq} stopped at ${where} — ${depth}, ${vars}`;
}

/** Last path segment, tolerant of both separators and of `undefined`. */
function basename(p: string | undefined): string {
    if (!p) {
        return '<unknown>';
    }
    const parts = p.split(/[\\/]/);
    return parts[parts.length - 1] || p;
}
