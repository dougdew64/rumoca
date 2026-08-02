# The three reports, and how they compose

**Purpose:** how the survey, fidelity and oracle reports compose — survey says *eligible*,
fidelity says *trustworthy*, oracle says *finding*.
**Status:** authority for the corpus list (#52) and the oracle test (#43).
**Read when:** before building the corpus list or designing the oracle test. It constrains how the
oracle is *designed*, not merely how its results are shown.

Agreed with Doug 2026-07-31. *(Retitled 2026-08-01: the "Test mode" it was written for was
dropped in favour of one corpus list with a filter — see #52. **Nothing about the reports
changed**, which is the point: the composition below was always about the data, and it is what
survived the mode.)* **The design authority for the corpus list
(`docs/ideas.md` #52) and for the oracle test (#43)** — consult it before building either,
because the composition below is a constraint on how the oracle test is *designed*, not
merely on how its results are displayed.

## The three

| Report | Question | Produced by | Ends in |
|---|---|---|---|
| **Survey** / capability map | how much of MSL does **Rumoca** compile, and where does it stop | `examples/survey_msl.rs` | a published table |
| **Fidelity** | does **HRW** agree with Rumoca, per model | F1-F9 (`src/fidelity.rs`, `worker.rs`) | **a commit** — we own HRW |
| **Oracle** | does **Rumoca** agree with **System Modeler** | not built (#43) | **a PR** — we do not own Rumoca |

Doug's framing of the asymmetry, which is the right one: *"Just because we identify oracle
mismatches, we don't get to fix those."* Ownership decides what a row's resolution looks
like, and that difference propagates all the way into the UI.

## Fidelity is what makes an oracle finding admissible

**The most important thing on this page.** Take a model whose Rumoca answer differs from
System Modeler's. Is that a Rumoca bug?

**Not knowable until HRW is known to have told the truth about what Rumoca did.** If the
same model has a fidelity failure, the "mismatch" may be the instrument lying rather than
the compiler erring.

| Survey | Fidelity | Oracle | What it means |
|---|---|---|---|
| compiles | **green** | **mismatch** | **high-confidence upstream bug — worth filing** |
| compiles | *failing* | mismatch | **inadmissible.** Fix HRW, re-run, re-judge |
| compiles | green | match | Rumoca and System Modeler agree here |
| fails | — | — | a capability gap; the survey's business, not the oracle's |

So the three chain: **survey → eligible; fidelity → trustworthy; oracle → findings.**

This reframes why the fidelity work matters. It was justified as credibility with
maintainers (`docs/upstream-strategy.md`). It is more than that — **without it every
mismatch needs hand-adjudication, which is precisely the triage cost that ruled out a
compile census in `docs/ideas.md` #51.** Fidelity is what buys oracle findings their
cheapness.

### The design constraint that falls out

**All three reports must be joinable, and `name` is the join key** — the fully qualified
model name, spelled identically. The shared first-four-columns decision (`name`, `kind`,
`outcome`, `message`) was made so one loader could read all three; it turns out to matter
more than that, because `name` is what lets the admissibility table above be *computed*
rather than eyeballed.

**Design the oracle test to emit that same key.** A report keyed by file path, or by a
System Modeler model identifier, cannot be joined and the table above becomes manual work.

## Fidelity's steady state is green, and green is the deliverable

Doug's expectation was that the fidelity report is transient — run it, fix the bugs, it
reaches zero and stops being useful. Three reasons it is the opposite:

- **At MSL scale "fix them all immediately" will not hold.** 2,600 models can produce
  clustered failures, some of them shapes HRW does not handle yet and should not rush.
- **Green is its most valuable state.** A green fidelity report over the corpus *is* the
  evidence artifact the methodology doc rests on. It stops being a bug list and becomes a
  **certificate**.
- **It is a regression detector across rebases** — trigger 1 of the run policy in
  `docs/fidelity-plan.md`. After a Rumoca rebase it will not be green, and that is exactly
  when it earns its keep.

## Three reports, three default interactions

Because their steady states differ, **a list with no default filter is wrong** — which is not
the same as saying one list is wrong. Same widget, same loader, same layout
(`docs/ideas.md` #52), **different default filter per source**:

| Report | Interaction | LHS shows by default |
|---|---|---|
| Survey | **browse** — find a model by shape or package | the full list |
| Fidelity | **exceptions** — usually empty, and empty is success | failures only |
| Oracle | **worklist** — items whose state outlives the run | unfiled first |

A fidelity report rendering 2,600 green rows would bury its own good news; the summary
carries it instead, and the list carries the exceptions.

**This table is why the merge works rather than an argument against it** *(2026-08-01)*. Three
default filters over one widget is one widget; three widgets would be three things to keep in
sync. And the question that actually matters is the **join** — *fidelity-green **and**
oracle-mismatched*, which the section above makes a precondition of admissibility. Separate
views make that join something you do in your head.

## The oracle report needs state the run did not produce

It is the first report whose rows carry a *history*: **unfiled / filed (with a link) /
fixed upstream / won't-file (with a reason)**. A mismatch persists across runs until
someone upstream fixes it, so regeneration must **merge** with that state rather than
overwrite it.

Concretely: the generated table stays generated, and the per-item state lives in a small
side file keyed by model name, merged at load. Designing this in now is much cheaper than
discovering it after the first oracle run has been triaged by hand.

**Refined 2026-08-01, and the distinction is worth keeping sharp.** *Where the state lives* and
*where it is managed* are different questions:

- **It lives outside the generated table** — as above, and that is unchanged. Regeneration
  merges rather than overwrites.
- **The list may SHOW it as a column; the list is not where filing is managed.** Filing state
  is the status of a finding you intend to send upstream, not a property of a model, and it
  already has a home in [`upstream-issues.md`](upstream-issues.md) with a standing rule that
  **Claude never files.** A corpus browser that grows a workflow becomes a bug tracker, and
  this project already has the artifact that job belongs to.

## Click generates a draft; a human files it

Doug proposed clicking a mismatch to *generate and open a PR*. **Stop one step short.**

The standing rule is that Claude adds entries to `docs/upstream-issues.md` and never files
them, and `docs/upstream-strategy.md`'s caution is that a confident wrong diagnosis wastes a
maintainer's time and costs credibility permanently.

**An oracle mismatch is evidence, not a diagnosis.** Between the two sits real work:
minimize the reproducer, confirm it still reproduces against current upstream, check it is
not already filed, write expected-vs-actual. That work is exactly what made the two existing
`upstream-issues.md` entries credible.

So: **click → generate a draft** (into `upstream-issues.md` or the clipboard) pre-filled
with the reproducer, both toolchains' output, the Rumoca version, and the phase where they
diverge. Doug reviews, minimizes, files.

**Ninety percent of the value is the pre-filling.** The last ten percent is where the
credibility risk lives, and it is cheap to keep a human in it.
