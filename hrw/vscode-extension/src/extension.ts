/**
 * HRW Debugger Bridge — VS Code extension that arms/removes breakpoints on
 * a running debug session in response to file-based requests from the HRW
 * native app.
 *
 * Protocol:
 *   HRW writes `.hrw-bridge/breakpoint-request.json` →
 *   this extension reads it, calls `vscode.debug.addBreakpoints()` or
 *   `removeBreakpoints()`, deletes the request, and writes
 *   `.hrw-bridge/breakpoint-ack.json` so HRW knows the breakpoint is
 *   registered before spawning algorithm threads.
 *
 * Breakpoints accumulate per specimen. Changing the `specimen` field clears
 * all previously armed breakpoints. Duplicates (same file + line + condition)
 * are silently skipped. All armed breakpoints are auto-cleared when the
 * debug session ends.
 */
import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import {
    CHILD_LIMIT,
    DEBUG_STATE_FILE,
    DEFAULT_FRAME_LIMIT,
    DebugState,
    RawFrame,
    RawVariable,
    buildDebugState,
    describeStop,
} from './debug_state';
import {
    ArmVerdict,
    ExistingBreakpoint,
    describeVerdict,
    planAdd,
    summarize,
} from './arm_verdict';

const BRIDGE_DIR_NAME = '.hrw-bridge';
/** HRW writes requests here; this extension watches for changes. */
const REQUEST_FILE = 'breakpoint-request.json';
/** Written by this extension after processing a request, consumed by HRW. */
const ACK_FILE = 'breakpoint-ack.json';

interface BreakpointEntry {
    path: string;
    line: number;
    condition?: string;
}

interface BreakpointRequest {
    version: number;
    action?: 'add' | 'remove';
    specimen?: string;
    breakpoints: BreakpointEntry[];
}

let armedBreakpoints: vscode.Breakpoint[] = [];
let currentSpecimen: string | undefined;
let statusItem: vscode.StatusBarItem;

export function activate(context: vscode.ExtensionContext): void {
    const output = vscode.window.createOutputChannel('HRW Bridge');
    output.appendLine('HRW Debugger Bridge activated');

    statusItem = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Left, 0
    );
    statusItem.command = 'hrw.clearArmedBreakpoints';
    context.subscriptions.push(statusItem);

    context.subscriptions.push(
        vscode.debug.onDidTerminateDebugSession(() => {
            if (armedBreakpoints.length > 0) {
                clearArmed(output);
                output.appendLine('Debug session ended — cleared armed breakpoints');
            }
        })
    );

    const bridgeDir = findBridgeDir();
    if (!bridgeDir) {
        output.appendLine(
            'No .hrw-bridge directory found — will retry when workspace changes'
        );
        return;
    }

    output.appendLine(`Watching ${bridgeDir} for breakpoint requests`);

    const requestPath = path.join(bridgeDir, REQUEST_FILE);

    const watcher = vscode.workspace.createFileSystemWatcher(
        new vscode.RelativePattern(vscode.Uri.file(bridgeDir), REQUEST_FILE)
    );

    const handleRequest = (): void => {
        try {
            if (!fs.existsSync(requestPath)) { return; }
            const content = fs.readFileSync(requestPath, 'utf-8');
            const request: BreakpointRequest = JSON.parse(content);

            if (request.version !== 1) {
                output.appendLine(`Unknown request version: ${request.version}`);
                fs.unlinkSync(requestPath);
                return;
            }

            const action = request.action ?? 'add';

            // **The ack reports what is in place, not that the file was read.**
            // It used to be an unconditional `{"acked": true}` written here
            // regardless of outcome, which HRW consumed as `armed = acked`.
            // See `arm_verdict.ts` for the failure that made reachable.
            let verdict: ArmVerdict;
            if (action === 'remove') {
                handleRemove(request, output);
                verdict = summarize('remove', []);
            } else {
                verdict = handleAdd(request, output);
            }

            fs.unlinkSync(requestPath);

            output.appendLine(describeVerdict(verdict));

            // HRW polls for this file via `bridge::check_breakpoint_ack()`.
            const ackPath = path.join(bridgeDir, ACK_FILE);
            fs.writeFileSync(ackPath, JSON.stringify(verdict) + '\n');
        } catch (err) {
            output.appendLine(`Error: ${err}`);
        }
    };

    watcher.onDidCreate(handleRequest);
    watcher.onDidChange(handleRequest);
    context.subscriptions.push(watcher);

    if (fs.existsSync(requestPath)) {
        handleRequest();
    }

    // ---- Debug-state publishing (`docs/ideas.md` #72) ----
    //
    // Claude cannot see a debug session. #70 measured it: a stop produces no
    // location, no stack and no values, and nothing in Claude's tool surface
    // exposes them. What Claude *can* do is read a file — so the bridge
    // publishes what the Debug Adapter Protocol already reports.
    //
    // **A tracker is the only documented way to observe a stop.** There is no
    // `vscode.debug.onDidStop`; `onDidSendMessage` sees adapter→VS Code traffic,
    // which is where `stopped` and `continued` events live.
    let debugSeq = 0;
    const publish = (state: DebugState): void => {
        writeDebugState(bridgeDir, state, output);
    };
    const publishRunning = (session: vscode.DebugSession): void => {
        debugSeq += 1;
        publish(buildDebugState({
            seq: debugSeq,
            writtenAtMs: Date.now(),
            sessionId: session.id,
            sessionName: session.name,
            stopped: false,
        }));
    };

    context.subscriptions.push(
        vscode.debug.registerDebugAdapterTrackerFactory('*', {
            createDebugAdapterTracker(session: vscode.DebugSession) {
                return {
                    onDidSendMessage: async (message: any): Promise<void> => {
                        if (message?.type !== 'event') { return; }
                        if (message.event === 'stopped') {
                            debugSeq += 1;
                            await publishStop(
                                session, message.body, debugSeq, publish, output
                            );
                        } else if (message.event === 'continued') {
                            // Publishing the *running* state matters as much as
                            // publishing the stop: without it the last stop
                            // stays on disk looking current, and Claude would
                            // describe a position the program has left.
                            publishRunning(session);
                        }
                    },
                    onWillStopSession: (): void => publishRunning(session),
                };
            },
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('hrw.clearArmedBreakpoints', () => {
            clearArmed(output);
            vscode.window.showInformationMessage(
                'HRW: Cleared armed breakpoints'
            );
        })
    );

    context.subscriptions.push(output);
}

/**
 * Fetch the stack and locals for a stop, then publish.
 *
 * Three DAP round-trips: `stackTrace` → `scopes` for the innermost frame →
 * `variables` for its most local scope. **Each failure is recorded rather than
 * swallowed**, because `variables: null` with a reason is a usable answer while
 * `variables: []` on a failed fetch is a lie about the program.
 */
async function publishStop(
    session: vscode.DebugSession,
    body: any,
    seq: number,
    publish: (state: DebugState) => void,
    output: vscode.OutputChannel,
): Promise<void> {
    const threadId: number | undefined = body?.threadId;
    const reason: string | undefined = body?.reason;

    let frames: RawFrame[] = [];
    let variables: RawVariable[] | null = null;
    let variablesError: string | undefined;
    let variablesScope: string | undefined;
    // Declared out here so a throw inside the loop still publishes the tally —
    // the tally is the diagnostic, and losing it is how the first attempt at
    // this feature ended up unable to explain itself.
    const stackAttempts: string[] = [];
    let stackShape: string | undefined;
    let totalFrames: number | undefined;

    try {
        if (threadId === undefined) {
            variablesError = 'the stopped event carried no threadId';
        } else {
            // **Ask more than one way, and record what each way returned.**
            //
            // The first version sent `levels: 0`, which DAP defines as "all
            // frames" — and `cppvsdbg` answered with an empty array (measured
            // 2026-08-08, on a real breakpoint stop with a valid threadId). The
            // payload then said "the adapter reported no stack frames", which
            // was true, useless, and indistinguishable from a thread that had
            // none. Depth is the whole point here: for `augment_traced` the
            // stack IS the augmenting path (`docs/ideas.md` #72).
            //
            // So try an explicit depth first, then the adapter's own default
            // shape, and publish the tally either way. The cap lives in
            // `buildDebugState`, which declares truncation rather than hiding it.
            const shapes: Array<{ shape: string; args: Record<string, unknown> }> = [
                {
                    // **One MORE than we will publish, so saturation is visible.**
                    // Asking for exactly the limit and receiving exactly the limit
                    // is indistinguishable from a stack that happens to be that
                    // deep, and `buildDebugState` then computed `40 > 40 === false`
                    // and declared a capped stack complete. Over-requesting by one
                    // makes the cap detectable even from an adapter that omits
                    // DAP's `totalFrames`, which `cppvsdbg` does.
                    shape: `levels=${DEFAULT_FRAME_LIMIT + 1}`,
                    args: { threadId, startFrame: 0, levels: DEFAULT_FRAME_LIMIT + 1 },
                },
                { shape: 'threadId only', args: { threadId } },
                { shape: 'levels=0 (DAP "all")', args: { threadId, startFrame: 0, levels: 0 } },
            ];

            let rawFrames: any[] = [];
            for (const attempt of shapes) {
                // One failing shape must not abort the others — an adapter that
                // rejects a shape outright is a result, not an error.
                let got: any[] = [];
                try {
                    const reply = await session.customRequest('stackTrace', attempt.args);
                    got = reply?.stackFrames ?? [];
                    // DAP's own count of what EXISTS, which the array is not.
                    const total = reply?.totalFrames;
                    if (typeof total === 'number') {
                        totalFrames = total;
                    }
                    stackAttempts.push(
                        `${attempt.shape} -> ${got.length}` +
                        (typeof total === 'number' ? ` of ${total}` : ' (no totalFrames)'),
                    );
                } catch (err) {
                    stackAttempts.push(`${attempt.shape} -> threw ${err}`);
                }
                if (got.length > 0) {
                    rawFrames = got;
                    stackShape = attempt.shape;
                    break;
                }
            }

            frames = rawFrames.map((f: any): RawFrame => ({
                name: f?.name,
                path: f?.source?.path,
                line: f?.line,
                column: f?.column,
            }));

            const top = rawFrames[0];
            if (!top) {
                variablesError =
                    `no stack frames from any request shape: ${stackAttempts.join('; ')}`;
            } else {
                const scopeReply = await session.customRequest('scopes', {
                    frameId: top.id,
                });
                const scopes: any[] = scopeReply?.scopes ?? [];
                // Prefer a locals scope by name. Adapters disagree here —
                // cppvsdbg leads with "Locals", others put registers or globals
                // first — so falling back to the first scope is a guess and the
                // scope name is published so the guess is visible.
                const scope = scopes.find(
                    (s: any) => /local/i.test(s?.name ?? '')
                ) ?? scopes[0];

                if (!scope?.variablesReference) {
                    variablesError = 'no scope with a variablesReference';
                } else {
                    const varReply = await session.customRequest('variables', {
                        variablesReference: scope.variablesReference,
                    });
                    const topLevel: RawVariable[] = (varReply?.variables ?? []).map(
                        (v: any): RawVariable => ({
                            name: v?.name,
                            value: v?.value,
                            type: v?.type,
                            variablesReference: v?.variablesReference,
                        })
                    );

                    // **Expand one level.** `cppvsdbg` renders an aggregate as a
                    // summary — a slice comes back as `{ len=2 }` — so without
                    // this the payload can say `match_eq` has two slots and not
                    // what is in them. Those contents are the partial
                    // permutation (`docs/ideas.md` #72, and matching.md Act 4),
                    // which is the single most valuable thing here.
                    //
                    // One level only, and bounded. The graph is arbitrarily deep
                    // — a `Vec<Option<usize>>` nests element → enum → payload —
                    // and following it unbounded would turn one stop into
                    // hundreds of round trips. Depth beyond this is a later
                    // decision, driven by a question it fails to answer.
                    variables = await Promise.all(
                        topLevel.map(async (v) => {
                            if (!v.variablesReference) {
                                return v;
                            }
                            try {
                                const kidReply = await session.customRequest('variables', {
                                    variablesReference: v.variablesReference,
                                });
                                const all: RawVariable[] = (kidReply?.variables ?? []).map(
                                    (k: any): RawVariable => ({
                                        name: k?.name,
                                        value: k?.value,
                                        type: k?.type,
                                        variablesReference: k?.variablesReference,
                                    })
                                );
                                const kept = all.slice(0, CHILD_LIMIT);
                                return {
                                    ...v,
                                    children: kept,
                                    childrenTruncated: all.length > kept.length,
                                };
                            } catch (err) {
                                // An aggregate that refused to expand keeps its
                                // summary. `children` stays absent, which means
                                // "not fetched" — never an empty array, which
                                // would claim it had no elements.
                                output.appendLine(
                                    `expand ${v.name} failed: ${err}`
                                );
                                return v;
                            }
                        })
                    );
                    variablesScope = scope.name;
                }
            }
        }
    } catch (err) {
        // Stays null. An adapter that refused the request must not be
        // indistinguishable from a frame that genuinely has no locals.
        variablesError = `${err}`;
    }

    const state = buildDebugState({
        seq,
        writtenAtMs: Date.now(),
        sessionId: session.id,
        sessionName: session.name,
        stopped: true,
        reason,
        threadId,
        frames,
        variables,
        variablesError,
        variablesScope,
        stackAttempts,
        stackShape,
        totalFrames,
    });
    publish(state);
    // Must-fire: the channel says what was published, so a silent failure to
    // capture cannot look like a session with nothing to report.
    output.appendLine(describeStop(state));
}

/**
 * Write the payload via temp file + rename.
 *
 * **Atomic deliberately.** Claude may read at any instant, and a torn read
 * either fails to parse or — worse — parses into something wrong. `rename`
 * within a directory is atomic on Windows and POSIX alike.
 *
 * Nothing deletes this file on shutdown, which is intentional: a reader is
 * required to check `writtenAtMs` (`isStale`) before trusting it, and that check
 * has to work anyway for the case where VS Code exits without warning.
 */
function writeDebugState(
    bridgeDir: string,
    state: DebugState,
    output: vscode.OutputChannel,
): void {
    const dest = path.join(bridgeDir, DEBUG_STATE_FILE);
    const tmp = `${dest}.tmp`;
    try {
        fs.writeFileSync(tmp, JSON.stringify(state, null, 2) + '\n');
        fs.renameSync(tmp, dest);
    } catch (err) {
        output.appendLine(`Failed to write ${DEBUG_STATE_FILE}: ${err}`);
    }
}

/**
 * Snapshot `vscode.debug.breakpoints` as plain data for `arm_verdict`.
 *
 * **`enabled` is carried, and that is the point.** A disabled breakpoint stays
 * in this list, so a lookup that ignored the flag would report a line as covered
 * when nothing will stop there — one click of *Disable All Breakpoints* away.
 */
function existingBreakpoints(): ExistingBreakpoint[] {
    const out: ExistingBreakpoint[] = [];
    for (const bp of vscode.debug.breakpoints) {
        if (!(bp instanceof vscode.SourceBreakpoint)) { continue; }
        out.push({
            path: bp.location.uri.fsPath,
            // Requests are 1-based; VS Code positions are 0-based. Converted
            // here so `arm_verdict` never has to know which side it is on.
            line: bp.location.range.start.line + 1,
            condition: bp.condition ?? undefined,
            enabled: bp.enabled,
        });
    }
    return out;
}

/**
 * Add breakpoints from the request. Accumulates per specimen; clears on specimen change.
 *
 * Returns the verdict that becomes the ack — **what is now in place**, not what
 * this call happened to do. See `arm_verdict.ts` for why those differ.
 */
function handleAdd(request: BreakpointRequest, output: vscode.OutputChannel): ArmVerdict {
    if (request.specimen && request.specimen !== currentSpecimen) {
        clearArmed(output);
        currentSpecimen = request.specimen;
        output.appendLine(`Specimen changed to: ${request.specimen}`);
    }

    // Planned against a snapshot taken BEFORE anything is added, so each entry
    // is judged against the state HRW's request arrived in.
    const plan = planAdd(
        request.breakpoints,
        existingBreakpoints(),
        p => fs.existsSync(p)
    );

    const added: vscode.Breakpoint[] = [];
    for (let i = 0; i < plan.length; i += 1) {
        const entry = request.breakpoints[i];
        const verdict = plan[i];
        const label = `${path.basename(entry.path)}:${entry.line}`;

        switch (verdict.outcome) {
            case 'fileMissing':
                output.appendLine(`File not found: ${entry.path}`);
                break;
            case 'alreadyEnabled':
                output.appendLine(`Already armed: ${label} — skipped`);
                break;
            case 'disabled':
                // Loud, because this is the case that used to pass for success.
                output.appendLine(
                    `DISABLED breakpoint at ${label} — nothing will stop; not arming a duplicate`
                );
                break;
            case 'armed': {
                const location = new vscode.Location(
                    vscode.Uri.file(entry.path),
                    new vscode.Position(entry.line - 1, 0)
                );
                added.push(
                    new vscode.SourceBreakpoint(location, true, entry.condition ?? undefined)
                );
                const cond = entry.condition ? ` [${entry.condition}]` : '';
                output.appendLine(`Armed: ${label}${cond}`);
                break;
            }
        }
    }

    if (added.length > 0) {
        vscode.debug.addBreakpoints(added);
        armedBreakpoints.push(...added);
        updateStatus();
        vscode.window.showInformationMessage(
            `HRW: Armed ${added.length} breakpoint(s) (${armedBreakpoints.length} total)`
        );
    }

    return summarize('add', plan);
}

/** Remove breakpoints matching the request entries (by file URI + line). */
function handleRemove(request: BreakpointRequest, output: vscode.OutputChannel): void {
    const toRemove: vscode.Breakpoint[] = [];
    const toKeep: vscode.Breakpoint[] = [];

    for (const bp of armedBreakpoints) {
        if (matchesAnyEntry(bp, request.breakpoints)) {
            toRemove.push(bp);
        } else {
            toKeep.push(bp);
        }
    }

    if (toRemove.length > 0) {
        vscode.debug.removeBreakpoints(toRemove);
        armedBreakpoints = toKeep;
        updateStatus();
        for (const entry of request.breakpoints) {
            const label = `${path.basename(entry.path)}:${entry.line}`;
            output.appendLine(`Removed: ${label}`);
        }
    }
}

/** Check whether a VS Code breakpoint matches any entry in the request (by file URI + line). */
function matchesAnyEntry(bp: vscode.Breakpoint, entries: BreakpointEntry[]): boolean {
    if (!(bp instanceof vscode.SourceBreakpoint)) { return false; }
    const bpUri = bp.location.uri.toString();
    const bpLine = bp.location.range.start.line;
    return entries.some(entry => {
        const entryUri = vscode.Uri.file(entry.path).toString();
        return bpUri === entryUri && bpLine === (entry.line - 1);
    });
}

/** Locate the `.hrw-bridge` directory — checks both `hrw/.hrw-bridge` and `.hrw-bridge` at root. */
function findBridgeDir(): string | undefined {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders) { return undefined; }

    for (const folder of folders) {
        const root = folder.uri.fsPath;

        // hrw/.hrw-bridge (workspace root = Rumoca repo)
        const hrwBridge = path.join(root, 'hrw', BRIDGE_DIR_NAME);
        if (fs.existsSync(hrwBridge)) { return hrwBridge; }

        // .hrw-bridge at root (workspace root = hrw/)
        const rootBridge = path.join(root, BRIDGE_DIR_NAME);
        if (fs.existsSync(rootBridge)) { return rootBridge; }
    }

    return undefined;
}

/** Remove all armed breakpoints and reset specimen tracking. */
function clearArmed(output: vscode.OutputChannel): void {
    if (armedBreakpoints.length > 0) {
        vscode.debug.removeBreakpoints(armedBreakpoints);
        output.appendLine(`Cleared ${armedBreakpoints.length} armed breakpoint(s)`);
        armedBreakpoints = [];
        currentSpecimen = undefined;
        updateStatus();
    }
}

function updateStatus(): void {
    if (armedBreakpoints.length > 0) {
        statusItem.text = `$(debug-breakpoint) HRW: ${armedBreakpoints.length} armed`;
        statusItem.tooltip = 'Click to clear HRW armed breakpoints';
        statusItem.show();
    } else {
        statusItem.hide();
    }
}

export function deactivate(): void {
    armedBreakpoints = [];
    currentSpecimen = undefined;
}
