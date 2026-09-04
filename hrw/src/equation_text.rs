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
//! 1. **DAE, Events, Initialization** hold `dae::Equation` and `rumoca_core::Expression`
//!    — exactly what `expr_format` takes. [`for_events`] is the first; the others are a
//!    call each, which is why [`Rendered`] is generic over paths rather than shaped
//!    around Events.
//! 2. **Parse/Resolve/Instantiate/Typecheck** share `rumoca_core::Expression` (the AST
//!    carries the same `Binary`/`Unary` variants), so expression nodes are renderable
//!    there, but their *equations* are a different type and need a per-stage adapter.
//!    Flatten's are already rendered by the equation sheet — reuse, do not add.
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
}

/// Renderings for the Events stage, from the DAE the stage was built from.
///
/// The four collections `events_to_json` publishes, and **the keys here must match the
/// keys it emits** — `equations_f_c`, `relations`, `real_updates_f_z`,
/// `synthetic_root_conditions`. `every_events_rendering_lands_on_a_real_node` holds the
/// two together against a real compile, because a path typo is invisible: the tooltip
/// simply never appears.
pub fn for_events(d: &dae::Dae) -> Rendered {
    let mut out = Rendered::default();
    out.extend_indexed(
        "conditions.equations_f_c",
        &d.conditions.equations,
        expr_format::format_equation,
    );
    out.extend_indexed(
        "conditions.relations",
        &d.conditions.relations,
        expr_format::format_expr,
    );
    out.extend_indexed(
        "discrete_updates.real_updates_f_z",
        &d.discrete.real_updates,
        expr_format::format_equation,
    );
    out.extend_indexed(
        "events.synthetic_root_conditions",
        &d.events.synthetic_root_conditions,
        expr_format::format_expr,
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty DAE renders nothing, and that is a report rather than a gap.
    ///
    /// **The non-vacuity guard for every other test here.** A `Rendered` that was always
    /// empty would satisfy any assertion about a *missing* tooltip, which is the silent
    /// success this module's whole point is to avoid.
    #[test]
    fn nothing_renderable_produces_no_entries() {
        let r = for_events(&dae::Dae::default());
        assert!(r.is_empty(), "got {} entries: {r:?}", r.len());
        assert_eq!(r.get("conditions.equations_f_c[0]"), None);
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
