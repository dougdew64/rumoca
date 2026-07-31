//! **The MSL survey report** — its schema, its CSV codec, and the statistics
//! computed from it.
//!
//! One definition, three consumers: `examples/survey_msl.rs` writes it, the
//! planned Test mode (`docs/ideas.md` #52) reads it, and the published
//! capability map is generated from it. A second copy of the row shape or the
//! failure clustering would be a second thing to keep in sync, and the two bugs
//! the fidelity work found on 2026-07-31 were both exactly that — a
//! reimplementation drifting from its original.
//!
//! # Statistics are computed, never stored
//!
//! [`Summary`] is derived from the rows every time it is asked for. Storing the
//! numbers alongside the table would let them disagree with it, which is the
//! `end_to_end_tour.md` failure — stored prose describing a 7x7 incidence matrix
//! on a tab showing 48 equations, uncaught because nothing checked it.
//!
//! **The statistics are a noun; the statements about them are a verb.** HRW
//! computes the noun exactly; Claude supplies the interpretation when asked. So
//! there is no prose in this module, and none in the report.

use std::collections::BTreeMap;

/// One model's survey row.
///
/// Shape columns are `Option` and render blank: in a published table a `0`
/// meaning "not applicable" is indistinguishable from a `0` meaning "measured
/// zero".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurveyRow {
    // --- the four columns EVERY report shares, so Test mode has one loader
    //     (`docs/ideas.md` #52) ---
    pub name: String,
    pub kind: String,
    pub outcome: String,
    pub message: String,

    /// Top-level MSL package (`Electrical`, `Fluid`, …) — the grouping the LHS
    /// list needs, and cheaper to store than to re-derive per frame.
    pub package: String,
    pub secs: f64,

    // --- shape, when the compile succeeded ---
    pub n_equations: Option<usize>,
    pub n_states: Option<usize>,
    pub n_algebraic: Option<usize>,
    pub n_discrete: Option<usize>,
    pub n_parameters: Option<usize>,

    /// Structural analysis of the **raw** DAE: `ok`, `singular`, `error:…`.
    pub structural: String,
    /// Structural analysis **after the index-reduction funnel**: `ok`,
    /// `singular`, or empty when the raw system was already `ok`.
    ///
    /// **The column the first survey lacked, and its absence made the headline
    /// unreportable.** Without it `singular` conflates "high-index and fine once
    /// reduced" with "genuinely ill-posed" — 1,209 rows that could not be
    /// characterised either way. It is the same raw-vs-reduced distinction that
    /// broke F1's first tearing draft.
    pub index_reduced: String,

    pub n_blocks: Option<usize>,
    pub n_coupled: Option<usize>,
    pub largest_coupled: Option<usize>,

    // --- phenomena HRW has views for, so the sample can cover them ---
    /// Equations from `connect()` — the connection-expansion animation's subject.
    pub n_connect_eq: Option<usize>,
    /// Flow-sum equations, the other half of connection expansion.
    pub n_flow_eq: Option<usize>,
    /// `when` / `reinit` equations — the Events tab's subject.
    pub n_event_eq: Option<usize>,

    pub has_arrays: bool,
    pub max_depth: usize,
    pub n_functions: Option<usize>,
}

impl SurveyRow {
    pub const HEADER: &'static str = "name,kind,outcome,message,package,secs,n_equations,\
        n_states,n_algebraic,n_discrete,n_parameters,structural,index_reduced,n_blocks,\
        n_coupled,largest_coupled,n_connect_eq,n_flow_eq,n_event_eq,has_arrays,max_depth,\
        n_functions";

    pub fn to_csv(&self) -> String {
        let n = |v: Option<usize>| v.map_or(String::new(), |x| x.to_string());
        format!(
            "{},{},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            csv_field(&self.name), csv_field(&self.kind), csv_field(&self.outcome),
            csv_field(&self.message), csv_field(&self.package), self.secs,
            n(self.n_equations), n(self.n_states), n(self.n_algebraic),
            n(self.n_discrete), n(self.n_parameters),
            csv_field(&self.structural), csv_field(&self.index_reduced),
            n(self.n_blocks), n(self.n_coupled), n(self.largest_coupled),
            n(self.n_connect_eq), n(self.n_flow_eq), n(self.n_event_eq),
            self.has_arrays, self.max_depth, n(self.n_functions),
        )
    }

    /// Did the model reach a solvable system, by whatever route?
    ///
    /// **The honest reading of "usable"**, and not the same as `outcome ==
    /// "success"`: a compile that produces no equations, or a singular system
    /// index reduction cannot fix, has compiled without being simulatable.
    pub fn is_solvable(&self) -> bool {
        self.outcome == "success"
            && (self.structural == "ok" || self.index_reduced == "ok")
    }
}

/// Cluster a failure message into a root cause.
///
/// **The clustering is the analysis**, so it lives in code where it is one
/// definition and re-runnable, rather than in prose that would age against the
/// data. Order matters: the specific patterns must precede the generic ones.
pub fn cause(message: &str) -> &'static str {
    if message.is_empty() {
        "(no message)"
    } else if message.contains("Connections.branch")
        || message.contains("Connections.root")
        || message.contains("Connections.isRoot")
    {
        "Connections.* — overdetermined connectors (MLS 9.4)"
    } else if message.contains("PartialMedium")
        || message.contains("BaseProperties")
        || message.contains("Medium.")
    {
        "Media partial-package pattern"
    } else if message.starts_with("unbalanced model") {
        "unbalanced model"
    } else if message.starts_with("unresolved function call") {
        "unresolved function call (other)"
    } else if message.starts_with("unsupported equation form") {
        "unsupported equation form (other)"
    } else if message.starts_with("unresolved reference") {
        "unresolved reference"
    } else if message.starts_with("unresolved component dimension") {
        "unresolved dimension"
    } else if message.contains("array dimension mismatch") {
        "array dimension mismatch"
    } else if message.contains("algorithm") {
        "algorithm / assignment form"
    } else {
        "other"
    }
}

/// The top-level MSL package a qualified name sits in.
pub fn package_of(name: &str) -> String {
    let mut segs = name.split('.');
    match (segs.next(), segs.next()) {
        (Some("Modelica"), Some(p)) => p.to_owned(),
        (Some(root), _) => root.to_owned(),
        _ => String::new(),
    }
}

/// Which MSL sub-package a name sits in — a fairness signal, not a verdict.
///
/// An `Interfaces` class is usually partial and not meant to compile alone, so
/// counting its failure against Rumoca would be the misattribution
/// `docs/upstream-strategy.md` warns turns a capability map into a scorecard.
/// Recorded as raw data so the analysis has to show its working.
pub fn classify(name: &str) -> String {
    for marker in ["Examples", "Interfaces", "BaseClasses", "Internal", "Types", "Icons", "Tests"] {
        if name.split('.').any(|seg| seg == marker) {
            return marker.to_owned();
        }
    }
    "Component".to_owned()
}

/// Statistics derived from a set of rows. **Always computed, never stored.**
#[derive(Debug, Default, Clone)]
pub struct Summary {
    pub total: usize,
    /// `success` / `failed:Flatten` / … , descending by count.
    pub outcomes: Vec<(String, usize)>,
    /// Root cause → count, over failures only, descending.
    pub causes: Vec<(String, usize)>,
    /// Raw-DAE structural verdict among successes, descending.
    pub structural: Vec<(String, usize)>,
    /// Compiled **and** reached a solvable system — see [`SurveyRow::is_solvable`].
    pub solvable: usize,
    /// Singular raw, then `ok` after index reduction: healthy high-index models.
    pub rescued_by_reduction: usize,
    /// Singular raw and still singular reduced.
    pub still_singular: usize,
    /// Compiled to no equations at all.
    pub empty: usize,
    /// `kind` → (successes, total).
    pub by_kind: Vec<(String, usize, usize)>,
}

impl Summary {
    pub fn of(rows: &[SurveyRow]) -> Summary {
        let mut s = Summary { total: rows.len(), ..Default::default() };
        let mut outcomes: BTreeMap<&str, usize> = BTreeMap::new();
        let mut causes: BTreeMap<&str, usize> = BTreeMap::new();
        let mut structural: BTreeMap<String, usize> = BTreeMap::new();
        let mut by_kind: BTreeMap<&str, (usize, usize)> = BTreeMap::new();

        for r in rows {
            *outcomes.entry(r.outcome.as_str()).or_default() += 1;
            let e = by_kind.entry(r.kind.as_str()).or_default();
            e.1 += 1;
            if r.outcome == "success" {
                e.0 += 1;
                let verdict = if r.structural.starts_with("error:empty") {
                    "empty".to_owned()
                } else {
                    r.structural.clone()
                };
                if verdict == "empty" {
                    s.empty += 1;
                }
                if r.structural == "singular" {
                    if r.index_reduced == "ok" {
                        s.rescued_by_reduction += 1;
                    } else if !r.index_reduced.is_empty() {
                        s.still_singular += 1;
                    }
                }
                *structural.entry(verdict).or_default() += 1;
            } else {
                *causes.entry(cause(&r.message)).or_default() += 1;
            }
            if r.is_solvable() {
                s.solvable += 1;
            }
        }

        // Descending by count, then by name — a stable order, so a rendered
        // summary does not reshuffle between frames.
        let rank = |m: BTreeMap<String, usize>| {
            let mut v: Vec<(String, usize)> = m.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            v
        };
        s.outcomes = rank(outcomes.into_iter().map(|(k, v)| (k.to_owned(), v)).collect());
        s.causes = rank(causes.into_iter().map(|(k, v)| (k.to_owned(), v)).collect());
        s.structural = rank(structural);
        s.by_kind = {
            let mut v: Vec<(String, usize, usize)> =
                by_kind.into_iter().map(|(k, (ok, n))| (k.to_owned(), ok, n)).collect();
            v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
            v
        };
        s
    }
}

/// RFC-4180 quoting — messages carry commas and quotes freely.
pub fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// Split one RFC-4180 record into fields, honouring quotes and `""` escapes.
fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Parse a survey CSV.
///
/// **Column order is read from the header, not assumed**, so a column added in
/// the middle does not silently shift every value one place — which would be a
/// misrepresentation of exactly the kind these reports exist to catch. An
/// unknown column is ignored; a missing one leaves its field default.
pub fn parse_csv(text: &str) -> Vec<SurveyRow> {
    let mut lines = text.lines();
    let Some(header) = lines.next() else { return Vec::new() };
    let cols: Vec<String> = split_csv_line(header);
    let idx: BTreeMap<&str, usize> =
        cols.iter().enumerate().map(|(i, c)| (c.trim(), i)).collect();

    lines
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let f = split_csv_line(line);
            let get = |k: &str| -> String {
                idx.get(k).and_then(|i| f.get(*i)).cloned().unwrap_or_default()
            };
            let num = |k: &str| -> Option<usize> { get(k).parse().ok() };
            SurveyRow {
                name: get("name"),
                kind: get("kind"),
                outcome: get("outcome"),
                message: get("message"),
                package: get("package"),
                secs: get("secs").parse().unwrap_or(0.0),
                n_equations: num("n_equations"),
                n_states: num("n_states"),
                n_algebraic: num("n_algebraic"),
                n_discrete: num("n_discrete"),
                n_parameters: num("n_parameters"),
                structural: get("structural"),
                index_reduced: get("index_reduced"),
                n_blocks: num("n_blocks"),
                n_coupled: num("n_coupled"),
                largest_coupled: num("largest_coupled"),
                n_connect_eq: num("n_connect_eq"),
                n_flow_eq: num("n_flow_eq"),
                n_event_eq: num("n_event_eq"),
                has_arrays: get("has_arrays") == "true",
                max_depth: num("max_depth").unwrap_or(0),
                n_functions: num("n_functions"),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, outcome: &str, structural: &str, reduced: &str) -> SurveyRow {
        SurveyRow {
            name: name.to_owned(),
            kind: classify(name),
            package: package_of(name),
            outcome: outcome.to_owned(),
            structural: structural.to_owned(),
            index_reduced: reduced.to_owned(),
            ..Default::default()
        }
    }

    /// A row survives the CSV round trip, including a message full of the
    /// characters CSV reserves.
    #[test]
    fn a_row_round_trips_through_csv() {
        let mut r = row("Modelica.Fluid.Examples.Tank", "failed:Flatten", "", "");
        r.message = "unsupported equation form: Connections.branch(a.r, \"b\"), see [1,2]".into();
        r.n_equations = Some(42);
        r.largest_coupled = Some(7);
        r.has_arrays = true;
        r.max_depth = 3;

        let text = format!("{}\n{}\n", SurveyRow::HEADER, r.to_csv());
        let back = parse_csv(&text);
        assert_eq!(back.len(), 1);
        assert_eq!(back[0], r, "the row did not survive the CSV round trip");
    }

    /// **Columns are located by name.** A column inserted in the middle must not
    /// shift every later value one place — a misrepresentation of exactly the
    /// kind these reports exist to catch.
    #[test]
    fn a_column_inserted_in_the_middle_does_not_shift_the_others() {
        let text = "name,SOMETHING_NEW,outcome,n_equations\n\
                    Modelica.A,ignored,success,17\n";
        let rows = parse_csv(text);
        assert_eq!(rows[0].name, "Modelica.A");
        assert_eq!(rows[0].outcome, "success", "outcome read from the wrong column");
        assert_eq!(rows[0].n_equations, Some(17));
        assert_eq!(rows[0].kind, "", "a missing column leaves its field default");
    }

    /// "Compiled" is not "usable", and `is_solvable` is where that lives.
    #[test]
    fn solvable_means_reached_a_solvable_system_not_merely_compiled() {
        // Compiles, but has no equations at all.
        assert!(!row("M.A", "success", "error:empty system: no equations", "").is_solvable());
        // Compiles to a sound system directly.
        assert!(row("M.B", "success", "ok", "").is_solvable());
        // High-index: singular raw, solvable once reduced. THIS IS HEALTHY.
        assert!(row("M.C", "success", "singular", "ok").is_solvable());
        // Singular and index reduction could not fix it.
        assert!(!row("M.D", "success", "singular", "singular").is_solvable());
        // Did not compile.
        assert!(!row("M.E", "failed:Flatten", "", "").is_solvable());
    }

    /// The summary separates the three fates of a singular raw system, because
    /// lumping them is what made the first survey's headline unreportable.
    #[test]
    fn the_summary_separates_rescued_from_still_singular() {
        let rows = vec![
            row("M.A", "success", "ok", ""),
            row("M.B", "success", "singular", "ok"),
            row("M.C", "success", "singular", "ok"),
            row("M.D", "success", "singular", "singular"),
            row("M.E", "success", "error:empty system: no equations", ""),
            row("M.F", "failed:ToDae", "", ""),
        ];
        let s = Summary::of(&rows);
        assert_eq!(s.total, 6);
        assert_eq!(s.rescued_by_reduction, 2, "two high-index models were rescued");
        assert_eq!(s.still_singular, 1);
        assert_eq!(s.empty, 1);
        assert_eq!(s.solvable, 3, "ok + the two rescued — not the five that compiled");
        assert_eq!(s.outcomes[0], ("success".to_owned(), 5));
    }

    /// Failure clustering is stable and specific-before-generic.
    #[test]
    fn causes_cluster_specific_before_generic() {
        assert_eq!(
            cause("unsupported equation form: Connections.branch(port_p.reference, x)"),
            "Connections.* — overdetermined connectors (MLS 9.4)",
            "the generic `unsupported equation form` arm must not win",
        );
        assert_eq!(
            cause("unresolved function call: Modelica.Media.Interfaces.PartialMedium.foo"),
            "Media partial-package pattern",
        );
        assert_eq!(cause("unresolved reference: states"), "unresolved reference");
        assert_eq!(cause(""), "(no message)");
        assert_eq!(cause("something nobody predicted"), "other");
    }

    /// **A torn final line is recognisable**, which is what `--resume` relies on.
    ///
    /// Rows are flushed individually, so a kill mid-write can leave a partial
    /// last line. The survey's `load_partial` drops any row missing its `name` or
    /// `outcome` and re-surveys it — a resumed run that trusted a half-written
    /// row would carry a corrupt row into a published report.
    #[test]
    fn a_truncated_final_line_parses_as_incomplete_rather_than_plausible() {
        let text = format!(
            "{}\nModelica.A,Component,success,,Blocks,0.5,3,1,0,0,8,ok,,3,0,0,0,0,0,false,0,0\n\
             Modelica.B,Compon",
            SurveyRow::HEADER,
        );
        let rows = parse_csv(&text);
        assert_eq!(rows.len(), 2, "both lines parse; the torn one is filtered by its emptiness");
        assert_eq!(rows[0].outcome, "success");
        assert_eq!(rows[0].n_equations, Some(3));

        let complete: Vec<&SurveyRow> =
            rows.iter().filter(|r| !r.name.is_empty() && !r.outcome.is_empty()).collect();
        assert_eq!(complete.len(), 1, "the torn row must not survive the filter");
        assert_eq!(complete[0].name, "Modelica.A");
    }

    #[test]
    fn names_yield_their_package_and_kind() {
        assert_eq!(package_of("Modelica.Fluid.Examples.Tank"), "Fluid");
        assert_eq!(package_of("Complex"), "Complex");
        assert_eq!(classify("Modelica.Fluid.Examples.Tank"), "Examples");
        assert_eq!(classify("Modelica.Fluid.Interfaces.PartialTwoPort"), "Interfaces");
        assert_eq!(classify("Modelica.Electrical.Analog.Basic.Resistor"), "Component");
    }
}
