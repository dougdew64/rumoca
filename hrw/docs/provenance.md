# Provenance tags

**Purpose:** the tag vocabulary that separates what Claude verified from what it inferred.
**Status:** authority. A wrong tag fails a test, because a tag is a claim about
trustworthiness.
**Read when:** writing or editing anything in `compiler-phases/`, or citing a source file.

**How Claude marks what it knows from what it guessed.** `docs/ideas.md` #41 stage C.

The teaching database is Claude's own prose, months old, read back by a later session with
no memory of writing it. Untagged, it is indistinguishable from an authoritative outside
source — which is the echo chamber the whole arrangement exists to avoid. A tag says which
kind of claim a passage is, so a later session knows whether to trust it or re-check it.

## The three tags

Put one on its own line, immediately under the heading it governs.

```markdown
## How tearing picks a variable

*Verified 2026-07-30 against `crates/rumoca-phase-structural/src/tearing.rs`* — read while
instrumenting it. The greedy rule and the appearance counts are as described.
```

The italic part is the **tag**; the roman part after it says *what* was checked, and is
the half worth reading. A tag that only says "verified" tells a later session almost
nothing.

**A single asterisk.** `**Bold**` is ordinary emphasis — the lint reads bold prose
beginning "Verified" as prose, not as a claim about trustworthiness.

| Tag opens with | Means | Trusted on re-read? |
|---|---|---|
| `*Verified <date> against `<path>`*` | Claude read the code, or ran the tool, and checked | **Yes** |
| `*Cellier & Kofman, CSM §9.3.*` (any citation) | From the literature | **Yes**, against the source |
| `*Inference — not checked against the source.*` | Claude's reasoning | **No.** Re-check before relying |

Anything **untagged is `unverified` by default**: a lead, not a fact.

## Why `Verified` names a file

So the stage-B citation checker validates it for free. A `Verified` tag whose path has
moved fails `every_documented_source_path_exists`, which means **a tag cannot outlive the
thing it points at** — the failure mode that produced every stale record found so far.

## Upgrading is lazy, deliberately

There is **no audit project.** Tagging 9,000 lines up front would be a week of work
producing tags nobody had checked — the same mistake as writing tour prose ahead of use.

Instead: when a real question sends Claude into the source, the claims it *actually
checked while answering* get tagged on the way past. The database becomes trustworthy
exactly where it is used most, and stays honestly unmarked everywhere else.

That means **low coverage is not a defect**, and the lint deliberately does not fail on
untagged text. It fails on a tag that is malformed or points at a file that is gone —
because a wrong tag is worse than no tag.

## What a tag does not say

That the prose is *good*, or complete, or well-explained. Only that its factual claims
were checked against something, when, and against what.
