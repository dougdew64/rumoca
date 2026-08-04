//! Reading a compiler report without losing part of it silently.
//!
//! # The problem this exists to solve
//!
//! Every view in HRW that turns a Rumoca report into a picture parses JSON, and every
//! one of them was written with `filter_map` and `?`:
//!
//! ```ignore
//! let rows: Vec<DiffRow> = red.get("differentiated_rows")
//!     .and_then(Value::as_array)
//!     .map(|a| a.iter().filter_map(|r| Some(DiffRow {
//!         equation_origin: r.get("equation_origin")?.as_str()?.to_owned(),
//!         for_state: r.get("for_state")?.as_str()?.to_owned(),
//!     })).collect())
//!     .unwrap_or_default();
//! ```
//!
//! That reads as careful defensive parsing. What it does is **drop any entry it cannot
//! understand and leave no gap where the entry was** — the pane shows four
//! differentiated equations where the compiler produced five, and nothing on screen
//! gives the reader a reason to doubt it. A partial report is worse than a missing one
//! for exactly that reason, which is the finding behind `CLAUDE.md`'s rule that a pane
//! is a reporter and ships with a test.
//!
//! # Two cases that must not be collapsed
//!
//! - **Absent** — the compiler produced no such list. An already-index-1 model has no
//!   eliminations. Legitimate and common; renders as nothing, correctly.
//! - **Present but unreadable** — the key is there and this parser cannot understand
//!   it, or one entry inside it. That is a **defect**, and rendering it as an empty
//!   list tells the reader the compiler did nothing.
//!
//! `filter_map` makes these identical at the call site. [`parse_list`] separates them.
//!
//! # What it deliberately does not do
//!
//! It does not decide *whether* a `None` from the closure is a defect — the caller
//! knows that. A `filter_map` that genuinely filters (`match f.step { Scc { .. } =>
//! Some(..), _ => None }`, collecting one variant of an enum) is correct as it stands
//! and should **not** be converted; there, `None` means *this entry does not qualify*.
//! Introduced by the 2026-08-04 tech-debt sweep, which measured 31 such sites and
//! found both kinds.

use serde_json::Value;

/// Parse every element of a JSON list, **counting failures instead of dropping them**.
///
/// Appends a human-readable line to `problems` for each way the read fell short, and
/// returns whatever did parse — because a list with real entries carries real
/// information, and hiding it would trade one silent loss for another. What changes is
/// that the loss is stated.
///
/// See the module docs for why `key`-absent is not recorded as a problem.
pub fn parse_list<T>(
    parent: &Value,
    key: &str,
    problems: &mut Vec<String>,
    parse: impl Fn(&Value) -> Option<T>,
) -> Vec<T> {
    let Some(raw) = parent.get(key) else {
        // Absent. The compiler had nothing to say here, which is not a problem.
        return Vec::new();
    };
    let Some(arr) = raw.as_array() else {
        problems.push(format!("`{key}` is present in the report but is not a list"));
        return Vec::new();
    };
    let mut out = Vec::with_capacity(arr.len());
    let mut bad = 0usize;
    for element in arr {
        match parse(element) {
            Some(v) => out.push(v),
            None => bad += 1,
        }
    }
    if bad > 0 {
        problems.push(format!(
            "{bad} of {} `{key}` entries could not be read \u{2014} they are missing from \
             the list below",
            arr.len(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_key_is_not_a_problem() {
        let mut problems = Vec::new();
        let out: Vec<u64> = parse_list(&serde_json::json!({}), "xs", &mut problems, |v| v.as_u64());
        assert!(out.is_empty());
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn an_unreadable_entry_is_counted() {
        let mut problems = Vec::new();
        let doc = serde_json::json!({ "xs": [1, "no", 3] });
        let out: Vec<u64> = parse_list(&doc, "xs", &mut problems, |v| v.as_u64());
        assert_eq!(out, vec![1, 3], "what parsed is still returned");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("1 of 3"), "{:?}", problems[0]);
    }

    #[test]
    fn a_key_that_is_not_a_list_is_a_problem() {
        let mut problems = Vec::new();
        let doc = serde_json::json!({ "xs": 7 });
        let out: Vec<u64> = parse_list(&doc, "xs", &mut problems, |v| v.as_u64());
        assert!(out.is_empty());
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("not a list"), "{:?}", problems[0]);
    }
}
