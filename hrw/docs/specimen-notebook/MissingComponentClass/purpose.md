# MissingComponentClass — the other kind of missing name

**Deliberately broken. Do not fix.** Read it beside `UndefinedRef`; neither is worth much alone.

```modelica
NoSuchBlock part;   // a class that does not exist
```

## What it demonstrates

Modelica looks up a **class** and a **component** by different rules, so the compiler can say
*which kind* of name it failed to find:

| Specimen | Missing thing | Resolve diagnostic |
|---|---|---|
| `UndefinedRef` | a variable in an equation | `unresolved component reference` |
| **`MissingComponentClass`** | a component's type | `unresolved type reference` |

**Both stop in exactly the same place** — flagged at Resolve, `Failed` at Flatten, same message.
The pair exists because the *diagnostic* differs while the *outcome* does not.

## The mistake this specimen corrected

Its first draft claimed *"Breaks at: INSTANTIATE"*, reasoning that a missing class must be a
different phase's problem from a missing variable — building the component tree rather than
resolving an identifier.

**That was wrong, and `cargo run -p hrw --example failure_map` said so before any lab was
written about it.** To Rumoca both are name resolution. Had the check been skipped, the lab
would have taught a phase boundary that does not exist, and it would have passed its own link
check the whole time.

This is the `oracle first for specimens` rule applied to HRW's own behaviour: **find out what
actually happens before concluding anything**, including when the thing you are reasoning about
is the compiler you are studying.

## What to look at

The **Resolve** tab for each specimen, side by side. Same stage, same outcome, different sentence
— and the sentence is the only place the distinction survives.

## Verified

`cargo run -p hrw --example failure_map` — identical `Failed` stage and message to `UndefinedRef`.
