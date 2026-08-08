/**
 * Tests for the arm verdict — the ack payload (`docs/ideas.md` #71, #74, #75).
 *
 * These exercise the real module, which imports no `vscode`; that split is the
 * whole reason the decision logic was moved out of `extension.ts`.
 *
 * **What is under test is one claim**: does an ENABLED breakpoint now exist at
 * every requested line? The predecessor answered "I read your file" and HRW
 * consumed it as "a breakpoint exists", which is `#71`'s fiction — so a wrong
 * answer here does not merely mislead a log, it makes HRW announce a stepped
 * session that will never stop.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
    findExisting,
    planAdd,
    summarize,
    describeVerdict,
} from '../out/arm_verdict.js';

const ANCHOR = 'c:\\repo\\crates\\rumoca-phase-structural\\src\\live_trace.rs';
const allFilesExist = () => true;

/** One requested breakpoint at the live-trace anchor. */
function request(line = 173, condition) {
    return [{ path: ANCHOR, line, condition }];
}

function existing({ line = 173, enabled = true, condition, path = ANCHOR } = {}) {
    return [{ path, line, condition, enabled }];
}

describe('a disabled breakpoint is not a breakpoint', () => {
    // THE BUG THIS FILE EXISTS FOR. One click of VS Code's "Disable All
    // Breakpoints" and the old code reported the line as covered, acked true,
    // and HRW ran the algorithm to completion with no stop and no notice.
    it('reports the line as NOT covered', () => {
        const plan = planAdd(request(), existing({ enabled: false }), allFilesExist);
        assert.equal(plan[0].outcome, 'disabled');

        const v = summarize('add', plan);
        assert.equal(v.breakpointPresent, false);
        assert.match(v.reason, /DISABLED/);
        assert.match(v.reason, /173/);
    });

    // **The remedy is a new session, not the enable checkbox.** The first
    // version of this message said "enable it, or use Enable All Breakpoints",
    // which Doug tried on 2026-08-08: the marker stayed hollow and nothing
    // stopped. `cppvsdbg` does not re-bind a released location (#74), and
    // disable-then-enable is the same one-way door as remove-then-add.
    // Advice that does not work is worse than no advice — it costs a debugging
    // session before the reader stops believing it.
    it('advises restarting the session, not re-enabling', () => {
        const v = summarize('add', planAdd(request(), existing({ enabled: false }), allFilesExist));
        assert.match(v.reason, /new session/i);
        assert.doesNotMatch(
            v.reason,
            /enable it|Enable All/i,
            'must not suggest a remedy that was measured not to work'
        );
    });

    it('does not count it as armed or already enabled', () => {
        const v = summarize('add', planAdd(request(), existing({ enabled: false }), allFilesExist));
        assert.equal(v.armed, 0);
        assert.equal(v.alreadyEnabled, 0);
        assert.equal(v.requested, 1);
    });
});

describe('an enabled breakpoint already there is a real yes', () => {
    // The #74 fix made this the NORMAL path: after the first Debug press the
    // anchor is already armed, so every later press correctly adds nothing.
    // If this regressed to "not present", live trace would break on press two.
    it('counts as covered without arming anything', () => {
        const plan = planAdd(request(), existing({ enabled: true }), allFilesExist);
        assert.equal(plan[0].outcome, 'alreadyEnabled');

        const v = summarize('add', plan);
        assert.equal(v.breakpointPresent, true);
        assert.equal(v.armed, 0);
        assert.equal(v.alreadyEnabled, 1);
        assert.equal(v.reason, undefined);
    });
});

describe('a fresh line is armed', () => {
    it('is covered after arming', () => {
        const v = summarize('add', planAdd(request(), [], allFilesExist));
        assert.equal(v.armed, 1);
        assert.equal(v.breakpointPresent, true);
    });
});

describe('a missing file cannot be covered', () => {
    it('says so rather than acking success', () => {
        const v = summarize('add', planAdd(request(), [], () => false));
        assert.equal(v.entries[0].outcome, 'fileMissing');
        assert.equal(v.breakpointPresent, false);
        assert.match(v.reason, /no such file/);
    });
});

describe('partial success is failure', () => {
    it('one disabled entry sinks a request that armed another', () => {
        const entries = [
            { path: ANCHOR, line: 173 },
            { path: ANCHOR, line: 111 },
        ];
        const plan = planAdd(entries, existing({ line: 111, enabled: false }), allFilesExist);
        const v = summarize('add', plan);

        assert.equal(v.armed, 1, 'line 173 really was armed');
        assert.equal(
            v.breakpointPresent,
            false,
            'but the request is not satisfied while any line is dead'
        );
    });
});

describe('a removal never claims a breakpoint is in place', () => {
    // HRW does not poll after a remove, but an ack that said "present" would be
    // a live lie the moment anything started polling.
    it('reports not-present with a reason', () => {
        const v = summarize('remove', []);
        assert.equal(v.breakpointPresent, false);
        assert.match(v.reason, /removal/);
    });
});

describe('an empty request is not vacuously satisfied', () => {
    // "Every one of zero lines is armed" is exactly the true-but-useless answer
    // this whole change exists to remove.
    it('reports not-present', () => {
        const v = summarize('add', []);
        assert.equal(v.breakpointPresent, false);
        assert.match(v.reason, /named no breakpoints/);
    });
});

describe('the condition is part of identity', () => {
    it('a conditional request is not covered by an unconditional breakpoint', () => {
        const plan = planAdd(
            request(173, 'frame_index == 7'),
            existing({ condition: undefined }),
            allFilesExist
        );
        assert.equal(plan[0].outcome, 'armed', 'a different condition is a different breakpoint');
    });

    it('matching conditions do cover', () => {
        const plan = planAdd(
            request(173, 'frame_index == 7'),
            existing({ condition: 'frame_index == 7' }),
            allFilesExist
        );
        assert.equal(plan[0].outcome, 'alreadyEnabled');
    });
});

describe('path comparison survives Windows casing', () => {
    it('matches a drive letter in the other case', () => {
        const plan = planAdd(
            request(),
            existing({ path: ANCHOR.replace('c:\\', 'C:\\') }),
            allFilesExist
        );
        assert.equal(plan[0].outcome, 'alreadyEnabled');
    });
});

describe('the version is declared', () => {
    // HRW distinguishes "this bridge cannot say" from "no breakpoint" by the
    // absence of this field. If it ever stopped being written, an old-format
    // reader would silently take the wrong branch.
    it('every verdict carries version 2 and acked', () => {
        const v = summarize('add', planAdd(request(), [], allFilesExist));
        assert.equal(v.version, 2);
        assert.equal(v.acked, true);
    });
});

describe('the log line agrees with the verdict', () => {
    it('names the failure when not armed', () => {
        const v = summarize('add', planAdd(request(), existing({ enabled: false }), allFilesExist));
        assert.match(describeVerdict(v), /NOT armed/);
    });

    it('reports coverage when armed', () => {
        const v = summarize('add', planAdd(request(), [], allFilesExist));
        assert.match(describeVerdict(v), /covered/);
    });
});

describe('findExisting ignores non-matching lines', () => {
    it('does not match a different line in the same file', () => {
        assert.equal(findExisting({ path: ANCHOR, line: 173 }, existing({ line: 174 })), undefined);
    });
});
