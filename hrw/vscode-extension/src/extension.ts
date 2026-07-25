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

const BRIDGE_DIR_NAME = '.hrw-bridge';
/** HRW writes requests here; this extension watches for changes. */
const REQUEST_FILE = 'breakpoint-request.json';
/** Written by this extension after processing a request, consumed by HRW. */
const ACK_FILE = 'breakpoint-ack.json';
/** Written by this extension when the user clicks an hrw:// deep link. */
const NAVIGATE_REQUEST_FILE = 'navigate-request.json';

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

    // Register the link provider and navigate command unconditionally —
    // these must work even before HRW creates the .hrw-bridge directory.
    context.subscriptions.push(
        vscode.languages.registerDocumentLinkProvider(
            { language: 'markdown', scheme: 'file' },
            new HrwLinkProvider()
        )
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('hrw.navigate', (args: { specimen?: string; stage?: string; path?: string[] }) => {
            if (!args?.stage && !args?.specimen) {
                output.appendLine('hrw.navigate: missing stage or specimen argument');
                return;
            }
            const dir = findBridgeDir();
            if (!dir) {
                vscode.window.showWarningMessage(
                    'HRW: No .hrw-bridge directory found — is HRW running?'
                );
                return;
            }
            const navPath = path.join(dir, NAVIGATE_REQUEST_FILE);
            const request: Record<string, unknown> = {};
            if (args.specimen) { request.specimen = args.specimen; }
            if (args.stage) { request.stage = args.stage; }
            request.path = args.path ?? [];
            fs.writeFileSync(navPath, JSON.stringify(request) + '\n');
            const label = args.specimen ? `load ${args.specimen}` : `${args.stage} / ${(args.path ?? []).join('/')}`;
            output.appendLine(`Navigate: ${label}`);
        })
    );

    const bridgeDir = findBridgeDir();
    if (!bridgeDir) {
        output.appendLine(
            'No .hrw-bridge directory found — breakpoint bridge inactive (deep links still work)'
        );
        context.subscriptions.push(output);
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

            if (action === 'remove') {
                handleRemove(request, output);
            } else {
                handleAdd(request, output);
            }

            fs.unlinkSync(requestPath);

            const ackPath = path.join(bridgeDir, ACK_FILE);
            fs.writeFileSync(ackPath, JSON.stringify({ acked: true }) + '\n');
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
 * Detect `hrw://Stage/path/segments` and `hrw://load/Specimen` URIs in
 * markdown files and turn them into clickable links that invoke `hrw.navigate`.
 */
class HrwLinkProvider implements vscode.DocumentLinkProvider {
    provideDocumentLinks(doc: vscode.TextDocument): vscode.DocumentLink[] {
        const links: vscode.DocumentLink[] = [];
        const re = /hrw:\/\/([A-Za-z_]+)(\/[A-Za-z0-9_.[\]-]*(?:\/[A-Za-z0-9_.[\]-]*)*)?/g;
        for (let i = 0; i < doc.lineCount; i++) {
            const line = doc.lineAt(i).text;
            let match: RegExpExecArray | null;
            while ((match = re.exec(line)) !== null) {
                const start = new vscode.Position(i, match.index);
                const end = new vscode.Position(i, match.index + match[0].length);
                const range = new vscode.Range(start, end);
                const first = match[1];
                const pathStr = (match[2] ?? '').replace(/^\//, '');
                const pathSegs = pathStr ? pathStr.split('/') : [];
                let navArgs: Record<string, unknown>;
                let tooltip: string;
                if (first.toLowerCase() === 'load' && pathSegs.length > 0) {
                    navArgs = { specimen: pathSegs[0] };
                    tooltip = `Open ${pathSegs[0]} in HRW`;
                } else {
                    navArgs = { stage: first, path: pathSegs };
                    tooltip = pathSegs.length > 0
                        ? `Navigate to ${first} / ${pathSegs.join('/')}`
                        : `Switch to ${first} tab`;
                }
                const args = encodeURIComponent(JSON.stringify([navArgs]));
                const target = vscode.Uri.parse(`command:hrw.navigate?${args}`);
                const link = new vscode.DocumentLink(range, target);
                link.tooltip = tooltip;
                links.push(link);
            }
        }
        return links;
    }
}

/** Add breakpoints from the request. Accumulates per specimen; clears on specimen change. */
function handleAdd(request: BreakpointRequest, output: vscode.OutputChannel): void {
    if (request.specimen && request.specimen !== currentSpecimen) {
        clearArmed(output);
        currentSpecimen = request.specimen;
        output.appendLine(`Specimen changed to: ${request.specimen}`);
    }

    const added: vscode.Breakpoint[] = [];
    for (const entry of request.breakpoints) {
        if (!fs.existsSync(entry.path)) {
            output.appendLine(`File not found: ${entry.path}`);
            continue;
        }

        if (isDuplicate(entry)) {
            const label = `${path.basename(entry.path)}:${entry.line}`;
            output.appendLine(`Already armed: ${label} — skipped`);
            continue;
        }

        const location = new vscode.Location(
            vscode.Uri.file(entry.path),
            new vscode.Position(entry.line - 1, 0)
        );
        const bp = new vscode.SourceBreakpoint(
            location,
            true,
            entry.condition ?? undefined
        );
        added.push(bp);

        const label = `${path.basename(entry.path)}:${entry.line}`;
        const cond = entry.condition ? ` [${entry.condition}]` : '';
        output.appendLine(`Armed: ${label}${cond}`);
    }

    if (added.length > 0) {
        vscode.debug.addBreakpoints(added);
        armedBreakpoints.push(...added);
        updateStatus();
        vscode.window.showInformationMessage(
            `HRW: Armed ${added.length} breakpoint(s) (${armedBreakpoints.length} total)`
        );
    }
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

/** Prevent duplicate breakpoints: true if an armed breakpoint already covers this entry. */
function isDuplicate(entry: BreakpointEntry): boolean {
    const entryUri = vscode.Uri.file(entry.path).toString();
    const entryLine = entry.line - 1;
    return armedBreakpoints.some(bp => {
        if (!(bp instanceof vscode.SourceBreakpoint)) { return false; }
        return bp.location.uri.toString() === entryUri
            && bp.location.range.start.line === entryLine
            && (bp.condition ?? undefined) === (entry.condition ?? undefined);
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
