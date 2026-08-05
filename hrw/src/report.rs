//! **One loader for all three reports.**
//!
//! `docs/reports.md` establishes that the survey, the fidelity report and the
//! oracle report share their first four columns — `name`, `kind`, `outcome`,
//! `message` — so Test mode (`docs/ideas.md` #52) needs one list widget rather
//! than three. This is that loader.
//!
//! # Why generic rather than three typed readers
//!
//! A typed reader per report means Test mode grows a third branch when the
//! oracle report arrives, and the shared-column convention is enforced only by
//! everyone remembering it. Reading generically makes the convention *checkable*
//! — `report::parse` on any of the three yields the same four fields, and a
//! report that broke the convention fails to load rather than rendering blanks.
//!
//! The typed readers still exist where the columns actually matter:
//! [`crate::survey::parse_csv`] produces `SurveyRow` for the stratification
//! work. This layer is for *display and navigation*, where the extra columns
//! only need to be shown, not interpreted.
//!
//! # `name` is a join key
//!
//! Not merely a label. An oracle mismatch is an admissible upstream finding only
//! when the same model is fidelity-green, and that judgement is computed by
//! joining reports on the fully-qualified model name (`docs/reports.md`).

use std::collections::BTreeMap;

/// One row of any report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReportRow {
    /// Fully-qualified model name. **The join key across reports.**
    pub name: String,
    /// The MSL sub-package (`Examples`, `Interfaces`, …) — grouping, and a
    /// fairness signal when reading failures.
    pub kind: String,
    /// What became of this model in *this* report's terms.
    pub outcome: String,
    /// One line: enough to cluster, short enough for a list.
    pub message: String,
    /// Every remaining column, in header order, so a report can carry whatever
    /// it likes without this layer knowing about it.
    pub extra: Vec<(String, String)>,
}

impl ReportRow {
    /// A named extra column, if the report has one.
    pub fn get(&self, column: &str) -> Option<&str> {
        self.extra
            .iter()
            .find(|(k, _)| k == column)
            .map(|(_, v)| v.as_str())
    }

    /// Is this row one the reader should be looking at?
    ///
    /// **Deliberately not `outcome == "ok"`.** Each report spells success
    /// differently — the survey says `success`, fidelity says `ok`, and the
    /// oracle will say `match` — so asking "is this an exception" centrally is
    /// what lets one list widget default to the right rows for each
    /// (`docs/reports.md`: browse / exceptions / worklist).
    pub fn is_exception(&self) -> bool {
        !matches!(self.outcome.as_str(), "success" | "ok" | "match")
    }
}

/// A loaded report: its columns, its rows, and whatever provenance was beside it.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Header order, so a viewer can show the extra columns as the report meant.
    pub columns: Vec<String>,
    pub rows: Vec<ReportRow>,
}

impl Report {
    /// Outcome → count, descending then alphabetical — a stable order, so a
    /// rendered tally does not reshuffle between frames.
    pub fn outcome_tally(&self) -> Vec<(String, usize)> {
        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
        for r in &self.rows {
            *m.entry(r.outcome.as_str()).or_default() += 1;
        }
        let mut v: Vec<(String, usize)> = m.into_iter().map(|(k, n)| (k.to_owned(), n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    /// The rows worth a reader's attention — see [`ReportRow::is_exception`].
    pub fn exceptions(&self) -> Vec<&ReportRow> {
        self.rows.iter().filter(|r| r.is_exception()).collect()
    }

    /// Does this report carry the four shared columns?
    ///
    /// A report that does not cannot be joined or listed, and saying so on load
    /// beats rendering a column of blanks.
    pub fn has_shared_columns(&self) -> bool {
        ["name", "kind", "outcome", "message"]
            .iter()
            .all(|c| self.columns.iter().any(|h| h == c))
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

/// Parse any report CSV.
///
/// Columns are located **by header name**, never by position, so a report that
/// adds a column in the middle still loads correctly — the misrepresentation a
/// positional reader would produce is exactly the kind these reports exist to
/// catch.
pub fn parse(text: &str) -> Report {
    let mut lines = text.lines();
    let Some(header) = lines.next() else {
        return Report::default();
    };
    let columns = split_csv_line(header);
    let at = |name: &str| columns.iter().position(|c| c.trim() == name);
    let (i_name, i_kind, i_outcome, i_message) =
        (at("name"), at("kind"), at("outcome"), at("message"));

    let rows = lines
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let f = split_csv_line(line);
            let take = |i: Option<usize>| i.and_then(|i| f.get(i)).cloned().unwrap_or_default();
            let shared = [i_name, i_kind, i_outcome, i_message];
            let extra = columns
                .iter()
                .enumerate()
                .filter(|(i, _)| !shared.contains(&Some(*i)))
                .map(|(i, c)| (c.trim().to_owned(), f.get(i).cloned().unwrap_or_default()))
                .collect();
            ReportRow {
                name: take(i_name),
                kind: take(i_kind),
                outcome: take(i_outcome),
                message: take(i_message),
                extra,
            }
        })
        .collect();

    Report { columns, rows }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURVEY: &str = "name,kind,outcome,message,package,n_equations\n\
        Modelica.A,Examples,success,,Blocks,17\n\
        Modelica.B,Component,failed:Flatten,\"unsupported form: f(a, \"\"b\"\")\",Fluid,\n";

    const FIDELITY: &str = "name,kind,outcome,message,checks_failed,n_violations\n\
        Modelica.A,Examples,ok,,,0\n\
        Modelica.C,Component,violations,\"F5: matched to an unreferenced unknown\",F5,3\n";

    /// **Both reports load through one parser**, which is the shared-column
    /// convention made checkable rather than merely agreed.
    #[test]
    fn every_report_shape_loads_through_the_same_parser() {
        for (label, text) in [("survey", SURVEY), ("fidelity", FIDELITY)] {
            let r = parse(text);
            assert!(r.has_shared_columns(), "{label} is missing a shared column");
            assert_eq!(r.rows.len(), 2, "{label}");
            assert_eq!(r.rows[0].name, "Modelica.A", "{label}");
            assert_eq!(r.rows[0].kind, "Examples", "{label}");
        }
    }

    /// Quoted fields carrying commas and escaped quotes survive.
    #[test]
    fn a_quoted_message_with_commas_and_quotes_survives() {
        let r = parse(SURVEY);
        assert_eq!(r.rows[1].message, r#"unsupported form: f(a, "b")"#);
        assert_eq!(r.rows[1].get("package"), Some("Fluid"));
        assert_eq!(
            r.rows[1].get("n_equations"),
            Some(""),
            "an empty measurement stays empty"
        );
    }

    /// Each report spells success its own way, and the exception filter knows
    /// all of them — which is what lets one widget default correctly per report.
    #[test]
    fn exceptions_are_recognised_across_reports() {
        assert_eq!(
            parse(SURVEY).exceptions().len(),
            1,
            "survey: only the failure"
        );
        assert_eq!(
            parse(FIDELITY).exceptions().len(),
            1,
            "fidelity: only the violation"
        );

        let oracle = "name,kind,outcome,message\nM.A,Examples,match,\nM.B,Examples,mismatch,x\n";
        assert_eq!(
            parse(oracle).exceptions().len(),
            1,
            "oracle: only the mismatch"
        );
    }

    /// A report missing a shared column says so rather than rendering blanks.
    #[test]
    fn a_report_without_the_shared_columns_is_reported_as_such() {
        let bad = parse("model,result\nA,ok\n");
        assert!(!bad.has_shared_columns());
        assert_eq!(
            bad.rows[0].name, "",
            "nothing is invented for a missing column"
        );
    }

    /// Extra columns are read by name, so inserting one shifts nothing.
    #[test]
    fn an_inserted_column_shifts_nothing() {
        let text = "name,NEW,kind,outcome,message,tail\nM.A,x,Examples,ok,,z\n";
        let r = parse(text);
        assert_eq!(
            r.rows[0].outcome, "ok",
            "outcome read from the wrong column"
        );
        assert_eq!(r.rows[0].get("NEW"), Some("x"));
        assert_eq!(r.rows[0].get("tail"), Some("z"));
    }

    #[test]
    fn the_outcome_tally_is_stable_and_descending() {
        let text = "name,kind,outcome,message\n\
            A,K,b,\nB,K,a,\nC,K,a,\nD,K,c,\n";
        assert_eq!(
            parse(text).outcome_tally(),
            vec![
                ("a".to_owned(), 2),
                ("b".to_owned(), 1),
                ("c".to_owned(), 1)
            ],
            "descending by count, then alphabetical",
        );
    }
}
