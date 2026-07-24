import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';

const BRIDGE_DIR_NAME = '.hrw-bridge';
const REQUEST_FILE = 'breakpoint-request.json';

interface BreakpointEntry {
    path: string;
    line: number;
    condition?: string;
}

interface BreakpointRequest {
    version: number;
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
                return;
            }

            // Clear all breakpoints when the specimen changes.
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

                const location = new vscode.Location(
                    vscode.Uri.file(entry.path),
                    new vscode.Position(entry.line - 1, 0)
                );
                const bp = new vscode.SourceBreakpoint(
                    location,
                    true,
                    entry.condition
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

            fs.unlinkSync(requestPath);
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
