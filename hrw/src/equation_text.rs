//! Human-readable equations for tree nodes, keyed by the node's own path.
//!
//! # The problem, measured on the node Doug was pointing at
//!
//! `conditions.equations_f_c[0]` in the Events tree is **99 lines of JSON, 10 levels
//! deep**. What it says is `c[1] = h <= 0`. Everything else is `span`,
//! `source: 18269282801443552466`, `def_id: null`, `generated: true` — faithful, and
//! ninety-nine lines of encoding around one line of substance.
//!
//! Doug, 2026-09-04: *"might it be possible for the events stage tree to use the IR for
//! each equation node to provide a complete, human-readable equation in a tooltip"*.
//!
//! # Why this is safe rather than a new place to be wrong
//!
//! **It renders the TYPED IR, never the JSON in the pane.** A JSON-to-text renderer would
//! be a second formatter that could disagree with the equation sheet, and each would look
//! right on its own — the invisible drift this repository keeps finding. The worker holds
//! `&rumoca_ir_dae::Dae` when it builds the Events tree, so the text comes from the same
//! `expr_format` functions `equation_sheet.rs` uses.
//!
//! **`expr_format`'s match has no catch-all.** Twenty `Expression::` arms, zero `_ =>`, so
//! coverage is enforced by the compiler: a new Rumoca expression variant breaks the build
//! rather than quietly rendering something plausible.
//!
//! **The text is HRW's, and says so.** Rumoca never emitted the string `c[1] = h <= 0`;
//! the tooltip is labelled a rendering, per *a derived view declares that it is derived*.
//!
//! # Scope, ruled 2026-09-04
//!
//! Three groups, and only the first is built:
//!
//! 1. **DAE and Events** hold `dae::Equation` and `rumoca_core::Expression` — exactly what
//!    `expr_format` takes. Both are built: [`for_events`] and [`for_dae`], whose keys are
//!    NOT the same shape, which is the trap that group carries.
//!
//!    **Initialization was in this group by assumption and is not.** Its tree is an IC
//!    *plan* from `rumoca-phase-structural`, so it needed [`for_ic_plan`] rather than a
//!    third call to the same helper. The DAE's initial equations live in the *DAE* tree.
//! 2. **Flatten is built** — [`for_flatten`] — and the AST stages are **deliberately
//!    not**, which reverses what this note first said.
//!
//!    The claim was that Flatten's equations "are already rendered by the equation
//!    sheet". They are not: the sheet renders **DAE** equations. And the AST stages,
//!    named first, turn out to be the *least* worth doing — an equation in the AST **is
//!    the source Doug already has open**, and reaching those nodes needs a walker
//!    mirroring serde's paths through ten levels of nesting. Highest risk, lowest payoff.
//!    Flatten is the reverse: `connect` expansion happens there, so its equations appear
//!    nowhere in the model text.
//! 3. **Solve lowering cannot**: it holds no expression trees at all, only a
//!    straight-line register program (`LoadY { dst, index }`, `Binary { dst, op, lhs,
//!    rhs }`). Rendering that means reconstructing an expression from instructions, where
//!    a plausible-but-wrong result is easy to produce and hard to notice. **Deliberately
//!    not attempted.**

use std::collections::HashMap;

use rumoca_ir_dae as dae;

use crate::expr_format;

/// Node path → the equation or expression at that node, rendered.
///
/// Paths are spelled as `bridge::describe_path` spells them, because that is what the
/// tree has in hand while drawing a row. `is_empty()` is the honest state for a stage
/// with nothing renderable, and produces no tooltip line rather than an empty one.
#[derive(Debug, Clone, Default)]
pub struct Rendered(HashMap<String, String>);

impl Rendered {
    /// The rendering for a node, or `None`.
    pub fn get(&self, path: &str) -> Option<&str> {
        self.0.get(path).map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Every (path, rendering) pair, so a test can resolve each key against real IR.
    ///
    /// A key that does not match what `describe_path` builds simply never matches, and no
    /// tooltip appears — indistinguishable from a node with nothing to render. That is
    /// checkable only from outside, hence this.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Render `items` into `self` under `base[i]`.
    ///
    /// One helper for every collection so a new one cannot get its own spelling of the
    /// index — the paths must match `describe_path` exactly or the lookup silently misses,
    /// which would look like "this node has no equation".
    fn extend_indexed<T>(&mut self, base: &str, items: &[T], render: impl Fn(&T) -> String) {
        for (i, item) in items.iter().enumerate() {
            let text = render(item);
            if !text.is_empty() {
                self.0.insert(format!("{base}[{i}]"), text);
            }
        }
    }

    /// Insert already-rendered text, keyed `base[i]` by position in the iterator.
    ///
    /// **For a 0.10.0 arena, where [`Self::extend_indexed`] cannot be used.**
    /// That method borrows a slice and renders each element, which needs the
    /// element to carry everything the rendering requires. An arena element does
    /// not: rendering one needs the `DaeView` as well, and the view only exists
    /// inside `inspect`, so the text has to be produced there and handed over
    /// finished.
    ///
    /// A `None` means *this index did not resolve* and is skipped rather than
    /// stored as an empty string — the same treatment an empty rendering already
    /// got, so a tooltip is absent rather than blank. The position is still
    /// consumed, so every later index keeps the ordinal serde wrote for it.
    fn extend_rendered(&mut self, base: &str, items: impl Iterator<Item = Option<String>>) {
        for (i, text) in items.enumerate() {
            if let Some(text) = text
                && !text.is_empty()
            {
                self.0.insert(format!("{base}[{i}]"), text);
            }
        }
    }
}

/// Renderings for the DAE stage, whose tree is the whole `Dae` serialized.
///
/// # The keys are serde's, and on v0.10.0 every one of them changed
///
/// `dae_stage` publishes `Stage::from_ser(d)`, so the paths are whatever `Dae`'s
/// `Serialize` produces. Until 0.10.0 that was **flat**, using Rumoca's
/// mathematical names: `f_x`, `f_z`, `f_m`, `f_c`, `relation`,
/// `initial_equations`, `synthetic_root_conditions`.
///
/// **None of those keys exists now.** A 0.10.0 `Dae` serializes as
/// `{schema_version, source_map, storage}`, with one arena per kind under
/// `storage` — `continuous_equation_operations`,
/// `initialization_equation_operations`, `discrete_real_equations`, `relations`,
/// `conditions`, `roots`, and twenty more. So every base below gained a
/// `storage.` prefix and most were renamed outright.
///
/// **The key names here were read off an actual serialization, not inferred
/// from the struct.** That is the same discipline the previous version of this
/// comment recorded, and for the same reason: a wrong key produces a map that
/// silently never matches — no tooltip anywhere, indistinguishable from a stage
/// with nothing renderable. `every_dae_rendering_lands_on_a_real_node` keeps
/// them honest against a real compile.
///
/// # What is rendered, and what is deliberately not
///
/// Residual equations and relations render, because they are expressions and
/// [`crate::dae_expr_format`] renders expressions faithfully.
///
/// **`storage.conditions` and `storage.roots` are left alone.** A 0.10.0
/// condition is not an expression at all — it is a `ConditionOperation` algebra
/// (`And`, `Or`, `Not`, `AnyRise`, …) over relations and clocks, and a root is a
/// relation paired with an activation. Rendering either as an equation would be
/// a category error. **They are the Events pane's business, and since 2026-11-03
/// it does render them** — `worker::events_from_condition_algebra` publishes each
/// condition as an algebra *term* rather than an equation, which is why this
/// function still leaves them alone rather than having gained two more arms.
pub fn for_dae(d: &dae::Dae) -> Rendered {
    let mut out = Rendered::default();
    d.inspect(|view| {
        // Each arena is walked by index so the `[i]` in the path is the same
        // ordinal serde wrote, which is what makes the two sides line up.
        out.extend_rendered(
            "storage.continuous_equation_operations",
            (0..view.continuous_equation_count()).map(|i| {
                view.continuous_equation(i)
                    .map(|equation| crate::dae_expr_format::format_equation(view, equation))
            }),
        );
        out.extend_rendered(
            "storage.initialization_equation_operations",
            (0..view.initialization_equation_count()).map(|i| {
                view.initialization_equation(i)
                    .map(|equation| crate::dae_expr_format::format_equation(view, equation))
            }),
        );
        out.extend_rendered(
            "storage.relations",
            view.relations().map(|(_id, relation)| {
                Some(crate::dae_expr_format::format_expr(
                    view,
                    relation.expression(),
                ))
            }),
        );
    });
    out
}

/// Renderings for the Flatten stage, from the flat model.
///
/// # Why Flatten and not the AST stages, which the plan named first
///
/// Group 2 was *"Parse/Resolve/Instantiate/Typecheck share `rumoca_core::Expression`, so
/// their expression nodes are renderable"*. True, and **near worthless**: an equation in
/// the AST **is the source Doug already has open**. `h <= 0` in `parse.json` is what he
/// typed. Reaching those nodes also needs a walker mirroring serde's paths through ten
/// levels of nesting — the highest path-mismatch risk in the feature, for the lowest
/// payoff.
///
/// Flatten is the opposite trade. Its equations are **not** the source: this is where
/// `connect` expansion happens, so `resistor.v = resistor.R * resistor.i` appears where
/// the model said `connect(...)`. That is the same reason `flat_node_lines` exists and
/// `equation_sheet` calls Flatten *"the one other tree worth this"*.
///
/// # What renders
///
/// `rumoca_ir_flat::Equation` holds a `residual: Expression`, so a flat equation is
/// `0 = residual` — which is what it *means*, and stating it as an equality rather than a
/// bare expression is why this does not just call `format_expr` and stop.
///
/// `assert_equations` and `initial_assert_equations` are `AssertEquation`, a different
/// type, and `structured_equations` is a family rather than an equation. Left alone: a
/// renderer for a type nobody has verified is how a mis-keyed map ships green.
pub fn for_flatten(m: &rumoca_ir_flat::Model) -> Rendered {
    let mut out = Rendered::default();
    let residual =
        |eq: &rumoca_ir_flat::Equation| format!("0 = {}", expr_format::format_expr(&eq.residual));
    out.extend_indexed("equations", &m.equations, residual);
    out.extend_indexed("initial_equations", &m.initial_equations, residual);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A DAE with no equations renders nothing, and that is a report rather than a gap.
    ///
    /// **The non-vacuity guard for every other test here.** A `Rendered` that was always
    /// empty would satisfy any assertion about a *missing* tooltip, which is the silent
    /// success this module's whole point is to avoid.
    ///
    /// *(Rewritten for v0.10.0. It used `for_events` over `Dae::default()`, and
    /// neither survives: the Events stage was disabled at the time and its
    /// re-model renders condition terms rather than equations, while a `Dae` is
    /// valid-by-construction now and has no `default()`. An empty `SourceMap` is
    /// the smallest well-formed model, so the subject moved to `for_dae`.)*
    #[test]
    fn nothing_renderable_produces_no_entries() {
        let empty = dae::Dae::construct(rumoca_core::SourceMap::new(), |_model| Ok(()))
            .expect("an empty model is well-formed");
        let r = for_dae(&empty);
        assert!(r.is_empty(), "got {} entries: {r:?}", r.len());
        assert_eq!(r.get("storage.continuous_equation_operations[0]"), None);
    }

    /// The index in a path is the collection index, and only present items are keyed.
    #[test]
    fn only_items_that_exist_are_keyed() {
        let mut r = Rendered::default();
        r.extend_indexed("a.b", &["x", "y"], |s| (*s).to_owned());
        assert_eq!(r.get("a.b[0]"), Some("x"));
        assert_eq!(r.get("a.b[1]"), Some("y"));
        assert_eq!(
            r.get("a.b[2]"),
            None,
            "no key past the end of the collection"
        );
        assert_eq!(r.len(), 2);
    }

    /// An empty rendering is dropped rather than stored as a blank tooltip.
    #[test]
    fn a_blank_rendering_is_not_stored() {
        let mut r = Rendered::default();
        r.extend_indexed("a.b", &["", "y"], |s| (*s).to_owned());
        assert_eq!(r.get("a.b[0]"), None, "a blank line is worse than no line");
        assert_eq!(r.get("a.b[1]"), Some("y"));
    }
}
