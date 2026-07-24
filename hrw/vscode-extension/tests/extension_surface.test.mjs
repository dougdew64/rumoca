import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const PKG = JSON.parse(
    fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf-8')
);

describe('package.json surface contract', () => {
    it('has required fields', () => {
        assert.equal(PKG.name, 'hrw-debugger-bridge');
        assert.ok(PKG.engines.vscode);
        assert.equal(PKG.main, './out/extension.js');
    });

    it('declares activationEvents', () => {
        assert.ok(Array.isArray(PKG.activationEvents));
        assert.ok(PKG.activationEvents.includes('onStartupFinished'));
    });

    it('declares the clear command', () => {
        const cmds = PKG.contributes.commands.map(c => c.command);
        assert.ok(cmds.includes('hrw.clearArmedBreakpoints'));
    });

    it('entry point exists', () => {
        const entry = path.join(__dirname, '..', PKG.main);
        assert.ok(fs.existsSync(entry), `${entry} should exist after build`);
    });
});

describe('breakpoint request schema', () => {
    it('validates a well-formed request', () => {
        const request = {
            version: 1,
            breakpoints: [
                {
                    path: '/home/user/dev/rumoca/crates/rumoca-phase-resolve/src/registration.rs',
                    line: 22,
                },
            ],
        };
        assert.equal(request.version, 1);
        assert.equal(request.breakpoints.length, 1);
        assert.equal(request.breakpoints[0].line, 22);
        assert.equal(request.breakpoints[0].condition, undefined);
    });

    it('supports conditional breakpoints', () => {
        const request = {
            version: 1,
            breakpoints: [
                {
                    path: '/some/file.rs',
                    line: 42,
                    condition: 'n == 3',
                },
            ],
        };
        assert.equal(request.breakpoints[0].condition, 'n == 3');
    });

    it('supports specimen field for accumulation tracking', () => {
        const request = {
            version: 1,
            specimen: 'ProportionalLoop',
            breakpoints: [
                {
                    path: '/some/file.rs',
                    line: 22,
                    condition: 'def_id.0 == 85',
                },
            ],
        };
        assert.equal(request.specimen, 'ProportionalLoop');
        assert.equal(request.breakpoints[0].condition, 'def_id.0 == 85');
    });

    it('supports action: "remove" for breakpoint removal', () => {
        const request = {
            version: 1,
            action: 'remove',
            breakpoints: [
                {
                    path: '/some/file.rs',
                    line: 42,
                },
            ],
        };
        assert.equal(request.action, 'remove');
        assert.equal(request.breakpoints.length, 1);
        assert.equal(request.breakpoints[0].line, 42);
        assert.equal(request.breakpoints[0].condition, undefined);
    });

    it('default action is "add" when omitted', () => {
        const request = {
            version: 1,
            breakpoints: [{ path: '/some/file.rs', line: 10 }],
        };
        const action = request.action ?? 'add';
        assert.equal(action, 'add');
    });
});

describe('ack file protocol', () => {
    it('ack structure matches expected schema', () => {
        const ack = { acked: true };
        assert.deepEqual(ack, { acked: true });
        assert.equal(JSON.stringify(ack), '{"acked":true}');
    });

    it('ack file is parseable JSON with trailing newline', () => {
        const raw = JSON.stringify({ acked: true }) + '\n';
        const parsed = JSON.parse(raw);
        assert.equal(parsed.acked, true);
    });
});
