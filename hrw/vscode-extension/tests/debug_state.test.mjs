/**
 * Tests for the debug-state payload Claude reads (`docs/ideas.md` #72).
 *
 * These exercise the real module rather than asserting on objects the test
 * itself built — `debug_state.ts` imports no `vscode`, which is the whole reason
 * it was split out. Contrast `extension_surface.test.mjs`, several of whose
 * cases construct a request literal and then assert its own fields.
 *
 * What is under test is the honesty of the payload, not its shape: a wrong
 * answer here becomes Claude confidently describing a program state that never
 * existed.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
    DEBUG_STATE_VERSION,
    buildDebugState,
    describeStop,
    isStale,
    isUnavailableValue,
} from '../out/debug_state.js';

const AT = Date.parse('2026-08-08T12:00:00.000Z');

/** Three nested `augment_traced` frames — a two-edge augmenting path. */
const FRAMES = [
    { name: 'augment_traced', path: 'C:\\r\\matching.rs', line: 189, column: 25 },
    { name: 'augment_traced', path: 'C:\\r\\matching.rs', line: 210, column: 17 },
    { name: 'maximum_matching_with_trace', path: 'C:\\r\\matching.rs', line: 123, column: 9 },
];

describe('a stop', () => {
    it('takes its location from the innermost frame', () => {
        const s = buildDebugState({
            seq: 7, writtenAtMs: AT, stopped: true, reason: 'breakpoint',
            threadId: 4, frames: FRAMES, variables: [],
        });
        assert.equal(s.version, DEBUG_STATE_VERSION);
        assert.equal(s.location.line, 189, 'innermost frame, not outermost');
        assert.equal(s.location.frame, 'augment_traced');
        assert.equal(s.frameCount, 3, 'depth is the augmenting path length');
        assert.equal(s.framesTruncated, false);
        assert.equal(s.reason, 'breakpoint');
        assert.equal(s.threadId, 4);
        assert.equal(s.writtenAtIso, '2026-08-08T12:00:00.000Z');
    });

    it('reports no location when the adapter gave no frames', () => {
        const s = buildDebugState({
            seq: 1, writtenAtMs: AT, stopped: true, frames: [], variables: null,
            variablesError: 'the adapter reported no stack frames',
        });
        assert.equal(s.location, null, 'absence is stated, not invented');
        assert.equal(s.frameCount, 0);
    });
});

describe('running is not a stale stop', () => {
    it('blanks location, frames and values even when frames are passed', () => {
        // The guard that matters: a `continued` event must not leave the
        // previous position on disk looking current.
        const s = buildDebugState({
            seq: 2, writtenAtMs: AT, stopped: false,
            frames: FRAMES, variables: [{ name: 'eq', value: '0' }],
            reason: 'breakpoint', threadId: 4,
        });
        assert.equal(s.stopped, false);
        assert.equal(s.location, null);
        assert.deepEqual(s.frames, []);
        assert.equal(s.frameCount, 0);
        assert.equal(s.variables, null);
        assert.equal(s.reason, null);
        assert.equal(s.threadId, null);
    });
});

describe('unavailable values are distinguishable from absent ones', () => {
    it('null survives as null, with the reason kept', () => {
        const s = buildDebugState({
            seq: 3, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: null, variablesError: 'adapter refused: no such scope',
        });
        assert.equal(s.variables, null, 'never coerced to []');
        assert.equal(s.variableCount, null);
        assert.match(s.variablesError, /adapter refused/);
        assert.equal(s.variablesTruncated, false);
    });

    it('an empty list means genuinely none', () => {
        const s = buildDebugState({
            seq: 4, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: [], variablesScope: 'Locals',
        });
        assert.deepEqual(s.variables, []);
        assert.equal(s.variableCount, 0, '0 is a fact; null is an unknown');
        assert.equal(s.variablesError, null);
        assert.equal(s.variablesScope, 'Locals');
    });
});

describe('caps are declared, never silent', () => {
    it('keeps the true frame count when truncating', () => {
        const deep = Array.from({ length: 50 }, (_, i) => ({
            name: 'augment_traced', path: 'm.rs', line: 100 + i,
        }));
        const s = buildDebugState({
            seq: 5, writtenAtMs: AT, stopped: true, frames: deep,
            variables: [], frameLimit: 10,
        });
        assert.equal(s.frames.length, 10);
        assert.equal(s.frameCount, 50, 'a shortened list must not read as complete');
        assert.equal(s.framesTruncated, true);
        assert.equal(s.location.line, 100, 'still the innermost frame');
    });

    it('keeps the true variable count when truncating', () => {
        const many = Array.from({ length: 25 }, (_, i) => ({
            name: `v${i}`, value: `${i}`,
        }));
        const s = buildDebugState({
            seq: 6, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: many, variableLimit: 5,
        });
        assert.equal(s.variables.length, 5);
        assert.equal(s.variableCount, 25);
        assert.equal(s.variablesTruncated, true);
    });
});

describe('staleness', () => {
    it('is false inside the window and true outside it', () => {
        const s = buildDebugState({ seq: 8, writtenAtMs: AT, stopped: true });
        assert.equal(isStale(s, AT + 1_000, 5_000), false);
        assert.equal(isStale(s, AT + 5_001, 5_000), true, 'past the window');
        assert.equal(isStale(s, AT, 5_000), false, 'written now');
    });
});

describe('the output-channel line', () => {
    it('names where execution is and how deep', () => {
        const s = buildDebugState({
            seq: 9, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: [{ name: 'eq', value: '0' }],
        });
        const line = describeStop(s);
        assert.match(line, /#9/);
        assert.match(line, /matching\.rs:189/, 'basename, both separators');
        assert.match(line, /augment_traced/);
        assert.match(line, /3 frame/);
        assert.match(line, /1 var/);
    });

    it('says values were unavailable rather than reporting none', () => {
        const s = buildDebugState({
            seq: 10, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: null, variablesError: 'timeout',
        });
        const line = describeStop(s);
        assert.match(line, /unavailable/);
        assert.match(line, /timeout/);
        assert.doesNotMatch(line, /0 var/, 'null must not render as zero');
    });

    it('does not claim a position while running', () => {
        const s = buildDebugState({ seq: 11, writtenAtMs: AT, stopped: false });
        const line = describeStop(s);
        assert.match(line, /running/);
        assert.doesNotMatch(line, /stopped at/);
    });
});

/**
 * The `stackTrace` request tally.
 *
 * **Measured 2026-08-08 and the reason these fields exist:** `cppvsdbg` returned
 * zero frames for `levels: 0`, which DAP defines as "all frames". The payload
 * could only say "the adapter reported no stack frames" — true, and
 * indistinguishable from a thread that genuinely had none. An empty result that
 * cannot say what was asked is not a measurement.
 */
describe('stack request attempts', () => {
    it('carries the tally and the winning shape', () => {
        const s = buildDebugState({
            seq: 12, writtenAtMs: AT, stopped: true, frames: FRAMES,
            stackAttempts: ['levels=40 -> 0', 'threadId only -> 3'],
            stackShape: 'threadId only',
        });
        assert.deepEqual(s.stackAttempts, ['levels=40 -> 0', 'threadId only -> 3']);
        assert.equal(s.stackShape, 'threadId only');
        assert.match(describeStop(s), /via threadId only/);
    });

    it('prints the whole tally when no shape worked, because that is the finding', () => {
        const s = buildDebugState({
            seq: 13, writtenAtMs: AT, stopped: true, frames: [],
            stackAttempts: ['levels=40 -> 0', 'threadId only -> 0'],
            variables: null, variablesError: 'no stack frames from any request shape',
        });
        assert.equal(s.stackShape, null);
        const line = describeStop(s);
        assert.match(line, /levels=40 -> 0/, 'a failed probe must be readable in the channel');
        assert.match(line, /threadId only -> 0/);
    });

    it('defaults to an empty tally rather than to null', () => {
        const s = buildDebugState({ seq: 14, writtenAtMs: AT, stopped: true, frames: FRAMES });
        assert.deepEqual(s.stackAttempts, []);
        assert.equal(s.stackShape, null);
    });

    it('keeps the tally even when not stopped, unlike the location', () => {
        const s = buildDebugState({
            seq: 15, writtenAtMs: AT, stopped: false,
            stackAttempts: ['levels=40 -> 0'],
        });
        assert.deepEqual(s.stackAttempts, ['levels=40 -> 0'], 'a probe result outlives the stop');
        assert.equal(s.location, null, 'but the position does not');
    });
});

/**
 * A local that is not live at the current line.
 *
 * **Measured 2026-08-08.** Stopped at `augment_traced`'s `for var in vars` loop
 * head, `cppvsdbg` reported four of twelve locals as the string
 * `"Variable is optimized away and not available."` — `var` unbound, `holder` in
 * an unreached arm, `can_augment` assigned later, `iter` the desugared iterator.
 * Every one is an honest absence at that line. **None is a value**, and
 * `variableCount: 12` overstated what was known by four.
 *
 * The profile is not the cause: `rumoca-phase-structural` is already
 * `opt-level = 0`.
 */
describe('unavailable locals', () => {
    it('recognises the adapter prose for absence', () => {
        assert.equal(
            isUnavailableValue('Variable is optimized away and not available.'),
            true,
        );
        assert.equal(isUnavailableValue('<optimized out>'), true);
        assert.equal(isUnavailableValue(''), true, 'empty is not a value either');
        assert.equal(isUnavailableValue(undefined), true);
    });

    it('does not discard a real value that merely mentions availability', () => {
        // The opposite failure, and equally bad: a false `available: false`
        // hides data Claude actually had.
        assert.equal(isUnavailableValue('"availability check passed"'), false);
        assert.equal(isUnavailableValue('0'), false);
        assert.equal(isUnavailableValue('{ len=2 }'), false);
    });

    it('marks each local and counts the dead ones', () => {
        const s = buildDebugState({
            seq: 16, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: [
                { name: 'eq', value: '0', type: 'unsigned __int64' },
                { name: 'var', value: 'Variable is optimized away and not available.' },
                { name: 'holder', value: 'Variable is optimized away and not available.' },
            ],
        });
        assert.equal(s.variableCount, 3);
        assert.equal(s.variablesUnavailable, 2);
        assert.deepEqual(
            s.variables.map((v) => [v.name, v.available]),
            [['eq', true], ['var', false], ['holder', false]],
        );
        assert.match(describeStop(s), /2 not live here/);
    });

    it('stays quiet about dead locals when there are none', () => {
        const s = buildDebugState({
            seq: 17, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: [{ name: 'eq', value: '0' }],
        });
        assert.equal(s.variablesUnavailable, 0);
        assert.doesNotMatch(describeStop(s), /not live/);
    });

    it('reports null rather than zero when nothing was fetched', () => {
        const s = buildDebugState({
            seq: 18, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: null, variablesError: 'no scope',
        });
        assert.equal(s.variablesUnavailable, null, 'zero would claim a clean read');
    });
});

/**
 * Expanding an aggregate.
 *
 * `cppvsdbg` renders a slice as `{ len=2 }`, so without one level of expansion
 * the payload can say `match_eq` has two slots and not what is in them — and
 * those contents ARE the partial permutation (`matching.md` Act 4).
 */
describe('aggregate expansion', () => {
    it('carries children and marks their availability too', () => {
        const s = buildDebugState({
            seq: 19, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: [{
                name: 'match_eq',
                value: '{ len=2 }',
                variablesReference: 7,
                children: [
                    { name: '[0]', value: 'Some(0)' },
                    { name: '[1]', value: 'Variable is optimized away and not available.' },
                ],
            }],
        });
        const v = s.variables[0];
        assert.equal(v.available, true, 'the summary itself is a real value');
        assert.deepEqual(
            v.children.map((c) => [c.name, c.available]),
            [['[0]', true], ['[1]', false]],
            'availability must recurse, or a dead element reads as data',
        );
    });

    it('distinguishes not-expanded from expanded-and-empty', () => {
        const s = buildDebugState({
            seq: 20, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: [
                { name: 'refused', value: '{ len=9 }', variablesReference: 3 },
                { name: 'genuinely_empty', value: '{ len=0 }', variablesReference: 4, children: [] },
            ],
        });
        assert.equal(
            s.variables[0].children, undefined,
            'absent children mean NOT FETCHED — an empty array would claim no elements',
        );
        assert.deepEqual(s.variables[1].children, []);
    });

    it('declares child truncation rather than shortening silently', () => {
        const s = buildDebugState({
            seq: 21, writtenAtMs: AT, stopped: true, frames: FRAMES,
            variables: [{
                name: 'big', value: '{ len=100 }', variablesReference: 5,
                children: [{ name: '[0]', value: 'Some(0)' }], childrenTruncated: true,
            }],
        });
        assert.equal(s.variables[0].childrenTruncated, true);
    });
});
