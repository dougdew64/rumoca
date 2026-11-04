//! Index-reduction summary — the search Rumoca ran, as a panel.
//!
//! ## What index reduction is, and what "index" counts
//!
//! A **differential-algebraic equation** system has an *index*: roughly, how many times
//! some of its equations must be differentiated before the system can be handed to an
//! ODE solver. Index 1 is what the solvers want. Above that, no amount of reordering
//! helps, because the problem is not the order — it is that some equation constrains
//! coordinates without saying anything about their rates.
//!
//! **A mechanical constraint is the canonical source.** Mesh two rotating bodies with
//! an ideal gear and each body has an angle and a velocity, so four states; but the
//! gear ratio fixes the second angle as a multiple of the first. Four states, two
//! freedoms. Something has to give, and reduction is what gives it.
//!
//! ## What this pane shows, and why it is a log rather than a pipeline
//!
//! v0.9.20's reduction was a **ten-step funnel** — a fixed sequence of named passes,
//! each with a before/after state count — and this module rendered it as such. v0.10.0
//! does something different enough that the old rendering had no successor: reduction
//! is a **search**.
//!
//! - **Two lanes, `Direct` and `Holonomic`**, alternating as a fixed point.
//! - **Candidates are discovered per proof boundary** (`DirectAdmissible`,
//!   `DirectConditional`, `Holonomic`), and a lane can discover none.
//! - **An attempt rebuilds and re-sorts the whole system**, so its outcome is a
//!   *measurement*: `Sorted`, `Reduced`, `Held`, `Raised`, or one of four refusals.
//!
//! So the panel is a reader for `worker::index_reduction_from_observation`'s payload,
//! grouped back into rounds. It invents nothing: the residue definition and the
//! arena-local-ordinal warning are **read out of the payload** rather than restated
//! here, so there is one copy of each.
//!
//! ## Three things the rendering must not let a reader conclude
//!
//! Each is a measured case from the corpus, and each would be a natural misreading of
//! a plainer table:
//!
//! 1. **`Held` is progress.** `Drivetrain`'s round 3 selects a candidate whose residue
//!    does not move, and round 4 then succeeds, because the *system* changed even
//!    though the number did not. Rendering `Held` as a failure would teach that round
//!    3 was wasted.
//! 2. **Reduction is not a size reduction.** `CapacitorLoop` is admitted with 1 state,
//!    13 algebraics and 14 equations before *and* after. A pane showing only counts
//!    would say nothing happened; what changed is one manifold constraint and the
//!    matching, so the manifold count is shown beside the sizes.
//! 3. **Zero candidates is not a refusal.** `CartesianPendulum` discovers *nothing* in
//!    either lane across three alternations. Rumoca does not decline to differentiate
//!    `x^2 + y^2 = L^2`; it finds nothing to try, which is a sharper statement of
//!    `docs/ideas.md` #83 than "index reduction does not work".
//!
//! ## Why this is a panel, not a canvas
//!
//! Unlike the spy plot and incidence views (spatial, custom-painted on a pan/zoom
//! canvas), a search log is *sequential*: rounds, each with attempts and an outcome. A
//! scrollable panel with collapsing sections is the natural fit — no coordinate
//! transforms, no hit-testing.

use eframe::egui;
use serde_json::Value;

use crate::json_read::parse_list;

/// How many rounds to leave expanded on arrival.
///
/// **Not all of them.** `GearWithBrake` logs 40 records across 5 rounds and
/// `Drivetrain` 29 across 6; a panel that opens with every attempt expanded is a wall,
/// and the thing worth reading first is the shape — how many rounds, which lanes, where
/// it ended. The *last* round is the interesting one when a system failed, so it opens
/// too.
const ROUNDS_OPEN_BY_DEFAULT: usize = 1;

/// One system's size, before or after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemSize {
    states: u64,
    algebraics: u64,
    equations: u64,
    /// Constraints reduction left on the state manifold. `None` for the raw system,
    /// which has none by definition.
    manifold: Option<u64>,
}

/// One attempted candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    lane: String,
    /// Rumoca's own name for the candidate's shape: `direct`, `holonomic`,
    /// `derivative_definition`, `auxiliary_state`.
    form: String,
    /// Rumoca's own name for what rebuilding measured.
    outcome: String,
    /// The residue the rebuild produced, where the outcome carries one. `None` for
    /// `sorted` (there is no remainder) and for the four refusals (nothing was built).
    residue: Option<u64>,
    /// `file:line` of the span that identifies this candidate, if it resolved.
    ///
    /// **The span, never the ordinals.** `state_ordinal` and friends index the rebuilt
    /// system of their own round, so a name derived from one would be wrong from round
    /// 2 onward — the payload's `ordinals_note` says so, and this pane obeys it.
    at: Option<String>,
    /// The constraint's own source text, if the span resolved to one.
    excerpt: Option<String>,
}

/// One candidate group's result: a proof boundary and how many it admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    lane: String,
    group: String,
    discovered: u64,
}

/// What reduction selected in a round, if anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    lane: String,
    form: String,
    /// `file:line` of the constraint this candidate came from, if it resolved.
    at: Option<String>,
    /// The constraint's own source text, if the span resolved to one.
    ///
    /// **The most useful line in the payload for a reader**, and the reason is that
    /// almost every constraint reduction uses is declared in a *library* file rather
    /// than in the model: `BenchActuator`'s is `phi = flange.phi -
    /// internalSupport.phi` in the MSL's `RotationalEMF`. Showing the file alone
    /// leaves the reader to go and open it.
    excerpt: Option<String>,
    residue_before: u64,
    /// `None` when the selection sorted the system: there is no residue after, because
    /// there is no unmatched remainder. Zero would be a measurement.
    residue_after: Option<u64>,
}

/// One lane iteration: where it started, what it looked at, what it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Round {
    lane: String,
    number: u64,
    residue: u64,
    unmatched_equations: Vec<String>,
    unmatched_unknowns: Vec<String>,
    groups: Vec<Group>,
    attempts: Vec<Attempt>,
    selection: Option<Selection>,
    /// The lane restarted from the unmodified system during this round.
    retried_pristine: bool,
}

impl Round {
    /// Candidates discovered across every proof boundary this round consulted.
    fn discovered(&self) -> u64 {
        self.groups.iter().map(|g| g.discovered).sum()
    }
}

/// Parsed index-reduction report, ready for rendering.
pub struct ReductionView {
    /// Rumoca's own word for how the run ended: `borrowed`, `sorted`, `singular`,
    /// `failure`, `discards_initial`.
    outcome: String,
    admitted: bool,
    /// The payload's own sentence about what that outcome means here. Read rather than
    /// composed, because `sorted` means two different things depending on the round
    /// count and only the publisher knows which.
    meaning: Option<String>,
    lanes: Vec<String>,
    before: Option<SystemSize>,
    /// `None` when the system was not admitted — there is no reduced system to size,
    /// and showing the input twice would read as "reduction changed nothing".
    after: Option<SystemSize>,
    rounds: Vec<Round>,
    /// The payload's definition of `residue`, shown beside the number.
    residue_definition: Option<String>,
    /// The payload's warning that candidate ordinals are arena-local.
    ordinals_note: Option<String>,
    /// Why preparation stalled, when it did.
    stalled: Option<String>,
    /// Parts of the payload this parser could not read.
    ///
    /// Rendered **above** the summary, because the lines below it are positive claims
    /// about the model — *"no candidate was discovered"*, *"2 rounds"* — and each
    /// becomes false the moment a record is lost in parsing.
    problems: Vec<String>,
}

impl ReductionView {
    /// Build from an index-reduction stage report.
    ///
    /// `None` when the payload carries no `search` array, which is the case for a
    /// stage that was never reached. A stage that *was* reached always has one, even
    /// when the run refused the system — a refusal is a result.
    pub fn from_report(report: &Value) -> Option<ReductionView> {
        report.get("search")?.as_array()?;
        let mut problems = Vec::new();

        // **Grouped from the flat log rather than from a nested payload.** The records
        // arrive in the order they were observed, which is the only order that is
        // faithful; the grouping is this reader's convenience and belongs here.
        let records: Vec<Value> = parse_list(report, "search", &mut problems, |v| {
            // A record without a `kind` cannot be placed at all, and dropping it
            // silently would shorten the search. `parse_list` counts it.
            v.get("kind").and_then(Value::as_str).map(|_| v.clone())
        });

        let mut rounds: Vec<Round> = Vec::new();
        let mut stalled = None;
        for record in &records {
            let kind = record["kind"].as_str().unwrap_or_default();
            match kind {
                "round" => rounds.push(Round {
                    lane: lane_of(record),
                    number: record["round"].as_u64().unwrap_or(0),
                    residue: record["residue"].as_u64().unwrap_or(0),
                    unmatched_equations: strings(&record["unmatched_equations"]),
                    unmatched_unknowns: record["unmatched_unknowns"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|u| match (u["name"].as_str(), u["kind"].as_str()) {
                                    (Some(n), Some(k)) => format!("{n} ({k})"),
                                    (Some(n), None) => n.to_owned(),
                                    _ => "(unnamed)".to_owned(),
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    groups: Vec::new(),
                    attempts: Vec::new(),
                    selection: None,
                    retried_pristine: false,
                }),
                // **Everything below belongs to the round above it.** A record arriving
                // before any round is announced rather than attached to a round it did
                // not come from — see `problems`.
                "candidates" | "attempt" | "selected" | "retried_pristine" => {
                    let Some(round) = rounds.last_mut() else {
                        problems.push(format!(
                            "a `{kind}` record arrived before any round, so it belongs \
                             to no lane iteration this pane can show"
                        ));
                        continue;
                    };
                    match kind {
                        "candidates" => round.groups.push(Group {
                            lane: lane_of(record),
                            group: record["group"].as_str().unwrap_or("?").to_owned(),
                            discovered: record["discovered"].as_u64().unwrap_or(0),
                        }),
                        "attempt" => round.attempts.push(Attempt {
                            lane: lane_of(record),
                            form: record["identity"]["form"]
                                .as_str()
                                .unwrap_or("?")
                                .to_owned(),
                            outcome: record["outcome"]["kind"].as_str().unwrap_or("?").to_owned(),
                            residue: record["outcome"]["residue"].as_u64(),
                            at: location_of(&record["identity"]["declared_at"]),
                            excerpt: excerpt_of(&record["identity"]["declared_at"]),
                        }),
                        "selected" => {
                            round.selection = Some(Selection {
                                lane: lane_of(record),
                                form: record["identity"]["form"]
                                    .as_str()
                                    .unwrap_or("?")
                                    .to_owned(),
                                at: location_of(&record["identity"]["declared_at"]),
                                excerpt: excerpt_of(&record["identity"]["declared_at"]),
                                residue_before: record["residue_before"].as_u64().unwrap_or(0),
                                residue_after: record["residue_after"].as_u64(),
                            });
                        }
                        _ => round.retried_pristine = true,
                    }
                }
                "stopped" => {
                    // The stop's own detail, where it has one beyond the outcome word.
                    if let Some(error) = record["outcome"]["error"].as_str() {
                        stalled = Some(error.to_owned());
                    }
                }
                other => problems.push(format!(
                    "a search record of kind `{other}` is not one this pane knows, so \
                     whatever it reports is not on screen"
                )),
            }
        }

        if let Some(error) = report["stalled"]["error"].as_str() {
            stalled = Some(error.to_owned());
        }

        Some(ReductionView {
            outcome: report["outcome"]["kind"]
                .as_str()
                .unwrap_or("unreported")
                .to_owned(),
            admitted: report["outcome"]["admitted"].as_bool().unwrap_or(false),
            meaning: report["outcome"]["meaning"].as_str().map(str::to_owned),
            lanes: strings(&report["lanes"]),
            before: size_of(&report["before"], false),
            after: size_of(&report["after"], true),
            rounds,
            residue_definition: report["residue_definition"].as_str().map(str::to_owned),
            ordinals_note: report["ordinals_note"].as_str().map(str::to_owned),
            stalled,
            problems,
        })
    }

    /// Parts of the payload this parser could not read.
    ///
    /// Public and separate from the rendering for the reason `Plot::problems` is:
    /// `egui_kittest` cannot reach a custom-painted cell, so what a test *can* reach
    /// has to be the data. Nothing here is in the paint path.
    #[must_use]
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// Rounds the run logged.
    #[must_use]
    pub fn n_rounds(&self) -> usize {
        self.rounds.len()
    }

    /// Candidates discovered across every round and proof boundary.
    #[must_use]
    pub fn n_discovered(&self) -> u64 {
        self.rounds.iter().map(Round::discovered).sum()
    }

    /// **Did reduction change the system without changing its size?**
    ///
    /// The `CapacitorLoop` case, asked as a question the pane can answer rather than
    /// left for a reader to notice: counts identical on both sides, and a manifold
    /// constraint to show for it. `false` when either side is missing, because then
    /// there is nothing to compare rather than nothing to report.
    #[must_use]
    pub fn changed_without_resizing(&self) -> bool {
        match (&self.before, &self.after) {
            (Some(b), Some(a)) => {
                b.states == a.states
                    && b.algebraics == a.algebraics
                    && b.equations == a.equations
                    && a.manifold.unwrap_or(0) > 0
            }
            _ => false,
        }
    }

    /// Draw the panel. `tracked` is the followed identifier, highlighted where it
    /// appears among the unmatched names.
    pub fn ui(&self, ui: &mut egui::Ui, tracked: Option<&str>) {
        for problem in &self.problems {
            ui.colored_label(ui.visuals().warn_fg_color, format!("\u{26a0} {problem}"));
        }
        if !self.problems.is_empty() {
            ui.add_space(6.0);
        }

        self.summary(ui);
        ui.add_space(8.0);
        self.sizes(ui);

        if self.rounds.is_empty() {
            ui.add_space(8.0);
            // **Not an empty table.** Zero rounds is a real and informative state, and
            // it means two different things — see the payload's own `meaning`, shown in
            // the summary above.
            ui.weak(
                "No reduction round ran. Either the system sorted as it stood, or \
                 normalizing it was enough.",
            );
            return;
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("The search").strong());
        if let Some(note) = &self.residue_definition {
            ui.weak(format!("residue = {note}"));
        }
        if let Some(note) = &self.ordinals_note {
            ui.weak(note.as_str());
        }
        ui.add_space(4.0);

        let last = self.rounds.len().saturating_sub(1);
        for (i, round) in self.rounds.iter().enumerate() {
            let open = i < ROUNDS_OPEN_BY_DEFAULT || i == last;
            self.round_ui(ui, i + 1, round, open, tracked);
        }

        if let Some(stalled) = &self.stalled {
            ui.add_space(8.0);
            ui.colored_label(ui.visuals().warn_fg_color, format!("Stalled: {stalled}"));
        }
    }

    /// The headline: how it ended, in how many rounds, through which lanes.
    fn summary(&self, ui: &mut egui::Ui) {
        let verdict = if self.admitted {
            egui::RichText::new(format!("Admitted \u{2014} {}", self.outcome)).strong()
        } else {
            egui::RichText::new(format!("Not admitted \u{2014} {}", self.outcome))
                .strong()
                .color(ui.visuals().warn_fg_color)
        };
        ui.label(verdict);
        if let Some(meaning) = &self.meaning {
            ui.label(meaning.as_str());
        }
        let lanes = if self.lanes.is_empty() {
            "no lane was entered".to_owned()
        } else {
            format!("lanes: {}", self.lanes.join(", "))
        };
        ui.weak(format!(
            "{} round(s), {} candidate(s) discovered, {lanes}",
            self.rounds.len(),
            self.n_discovered(),
        ));
    }

    /// Before and after, side by side.
    fn sizes(&self, ui: &mut egui::Ui) {
        egui::Grid::new("reduction_sizes")
            .num_columns(5)
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("").strong());
                for header in ["states", "algebraics", "equations", "manifold"] {
                    ui.label(egui::RichText::new(header).strong());
                }
                ui.end_row();

                for (label, size) in [("before", &self.before), ("after", &self.after)] {
                    ui.label(label);
                    match size {
                        Some(s) => {
                            ui.label(s.states.to_string());
                            ui.label(s.algebraics.to_string());
                            ui.label(s.equations.to_string());
                            match s.manifold {
                                Some(m) => ui.label(m.to_string()),
                                None => ui.weak("\u{2014}"),
                            };
                        }
                        // Four cells, so the grid keeps its shape and the absence reads
                        // as one row rather than as a truncated table.
                        None => {
                            for _ in 0..4 {
                                ui.weak("\u{2014}");
                            }
                        }
                    }
                    ui.end_row();
                }
            });

        if self.after.is_none() {
            ui.weak(
                "No reduced system: preparation did not admit this one, so there is \
                 nothing to show on the other side.",
            );
        } else if self.changed_without_resizing() {
            // **The `CapacitorLoop` sentence.** Without it, identical columns read as
            // "reduction did nothing", which is the opposite of what happened.
            ui.label(
                "Every count is unchanged, and the system still changed: reduction \
                 added a manifold constraint and a different matching. Reduction is \
                 not a size reduction.",
            );
        }
    }

    /// One round, collapsible.
    /// One round, collapsible. `pass` is its position in the log, counted from 1.
    ///
    /// # `round.number` repeats, and both the label and the id account for it
    ///
    /// **Rumoca numbers rounds within a lane pass, and the counter restarts when a lane
    /// is re-entered.** Measured on `CartesianPendulum`: its three records read round
    /// **1** [direct], round **1** [holonomic], round **1** [direct]. `Drivetrain` is
    /// the same shape — direct rounds 1 to 5, then a holonomic round 1.
    ///
    /// Two consequences, and the first is a defect this had until 2026-11-04. The
    /// `id_salt` was `(number, lane)`, so the pendulum's first and third rounds **had
    /// the same egui id** — one collapsing header's open state driving two sections.
    /// Found while authoring `docs/fixture-labs/index-reduction.md` against this pane,
    /// which is the testing the labs buy that no assertion here does.
    ///
    /// And the header leads with the pass, because *"Round 1"* appearing three times
    /// reads as the search going in circles. The log's **order** is the sequence; the
    /// number is a lane-local detail.
    fn round_ui(
        &self,
        ui: &mut egui::Ui,
        pass: usize,
        round: &Round,
        open: bool,
        tracked: Option<&str>,
    ) {
        let header = format!(
            "Pass {pass} \u{2014} round {} [{}], residue {}, {} candidate(s)",
            round.number,
            round.lane,
            round.residue,
            round.discovered(),
        );
        egui::CollapsingHeader::new(header)
            .id_salt(("reduction_round", pass))
            .default_open(open)
            .show(ui, |ui| {
                if round.retried_pristine {
                    ui.weak("The lane restarted from the unmodified system.");
                }

                self.unmatched_ui(ui, round, tracked);

                for group in &round.groups {
                    let line = format!(
                        "{} / {}: {} candidate(s)",
                        group.lane, group.group, group.discovered
                    );
                    if group.discovered == 0 {
                        // **Nothing to try, which is not a refusal.** Said in words,
                        // because a `0` in a list of counts reads as a rejection.
                        ui.weak(format!("{line} \u{2014} nothing to try here"));
                    } else {
                        ui.label(line);
                    }
                }

                if round.attempts.is_empty() && round.discovered() == 0 {
                    ui.weak(
                        "No attempt was made, because no candidate was found \u{2014} \
                         this lane had nothing admissible to rebuild with.",
                    );
                }
                for attempt in &round.attempts {
                    self.attempt_ui(ui, attempt);
                }

                if let Some(selection) = &round.selection {
                    let after = match selection.residue_after {
                        Some(r) => r.to_string(),
                        None => "sorted".to_owned(),
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "Selected: {} [{}], residue {} \u{2192} {after}",
                            selection.form, selection.lane, selection.residue_before,
                        ))
                        .strong(),
                    );
                    // **The constraint itself, under the selection.** This is what the
                    // reader is really being shown: one line of Modelica, usually from
                    // a library file they did not write, that made a state redundant.
                    if let Some(at) = &selection.at {
                        ui.weak(format!("      at {at}"));
                    }
                    if let Some(excerpt) = &selection.excerpt {
                        ui.label(egui::RichText::new(format!("      {excerpt}")).monospace());
                    }
                }
            });
    }

    /// What is still unmatched on both sides, with the followed identifier marked.
    fn unmatched_ui(&self, ui: &mut egui::Ui, round: &Round, tracked: Option<&str>) {
        for (label, names) in [
            ("unmatched equations", &round.unmatched_equations),
            ("unmatched unknowns", &round.unmatched_unknowns),
        ] {
            if names.is_empty() {
                continue;
            }
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!("{label}:"));
                for name in names {
                    // Equality on the leading token, not `contains`: a followed `a.v`
                    // must not light up `a.vx`. `identity-and-provenance.md` — no
                    // substring search decides identity.
                    let followed = tracked.is_some_and(|t| {
                        name == t || name.split(' ').next().is_some_and(|head| head == t)
                    });
                    if followed {
                        ui.label(
                            egui::RichText::new(name)
                                .strong()
                                .color(ui.visuals().hyperlink_color),
                        );
                    } else {
                        ui.label(name);
                    }
                }
            });
        }
    }

    /// One attempt, with `Held` rendered as progress.
    fn attempt_ui(&self, ui: &mut egui::Ui, attempt: &Attempt) {
        let residue = match attempt.residue {
            Some(r) => format!(" \u{2192} residue {r}"),
            None => String::new(),
        };
        let gloss = match attempt.outcome.as_str() {
            // **The three that need a word, because the bare name misleads.**
            "held" => " (the residue held, and the system changed \u{2014} progress)",
            "raised" => " (the residue grew, so this candidate was not taken)",
            "sorted" => " (this one sorted the system)",
            _ => "",
        };
        let line = format!(
            "  {} [{}]: {}{residue}{gloss}",
            attempt.form, attempt.lane, attempt.outcome,
        );
        match attempt.outcome.as_str() {
            "sorted" | "reduced" | "held" => ui.label(line),
            _ => ui.weak(line),
        };
        if let Some(at) = &attempt.at {
            ui.weak(format!("      at {at}"));
        }
        if let Some(excerpt) = &attempt.excerpt {
            ui.label(egui::RichText::new(format!("      {excerpt}")).monospace());
        }
    }
}

/// A record's lane, or a placeholder that cannot be mistaken for one.
fn lane_of(record: &Value) -> String {
    record["lane"].as_str().unwrap_or("?").to_owned()
}

/// A `declared_at` location as `file:line`, or `None` when it did not resolve.
///
/// **`None` is a real answer**: a span in no registered source refuses rather than
/// resolving against whichever file was to hand, which is `worker::span_location`'s
/// whole point. Rendering it as an empty string would read as a location of nothing.
fn location_of(declared_at: &Value) -> Option<String> {
    let file = declared_at["file"].as_str()?;
    match declared_at["line"].as_u64() {
        Some(line) => Some(format!("{file}:{line}")),
        None => Some(file.to_owned()),
    }
}

/// The source text a `declared_at` quotes, if the span resolved to one.
///
/// Separate from [`location_of`] because the two can differ in availability: a span
/// that resolves to a file and line always has a line, and `excerpt` is the slice of
/// that line the span actually covers — absent when the span pointed into no registered
/// source, which `worker::span_location` refuses rather than guessing at.
fn excerpt_of(declared_at: &Value) -> Option<String> {
    let excerpt = declared_at["excerpt"].as_str()?.trim();
    (!excerpt.is_empty()).then(|| excerpt.to_owned())
}

/// A JSON string array as owned strings. Absent and empty both give an empty list,
/// which is correct here: both mean *nothing to show*, and the caller says which.
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("(unnamed)").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// One side's size, or `None` when that side is absent.
///
/// `with_manifold` because only the reduced side has manifold constraints; publishing
/// `0` for the raw system would be a measurement of something that does not apply.
fn size_of(value: &Value, with_manifold: bool) -> Option<SystemSize> {
    if value.is_null() {
        return None;
    }
    Some(SystemSize {
        states: value["n_states"].as_u64()?,
        algebraics: value["n_algebraics"].as_u64()?,
        equations: value["n_equations"].as_u64()?,
        manifold: if with_manifold {
            value["n_manifold_constraints"].as_u64()
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A payload shaped like `worker::index_reduction_from_observation`'s.
    fn report(search: Value, after: Value) -> Value {
        json!({
            "rounds": 1,
            "outcome": { "kind": "sorted", "admitted": true, "meaning": "it sorted" },
            "lanes": ["direct"],
            "residue_definition": "both deficits summed",
            "ordinals_note": "arena-local to the rebuilt system",
            "before": { "n_states": 9, "n_algebraics": 88, "n_equations": 97 },
            "after": after,
            "search": search,
        })
    }

    fn round(number: u64, residue: u64) -> Value {
        json!({
            "kind": "round",
            "lane": "direct",
            "round": number,
            "residue": residue,
            "unmatched_equations": ["f_x[90] (SI.Angle phi)"],
            "unmatched_unknowns": [{ "name": "emf.w", "kind": "derivative" }],
        })
    }

    #[test]
    fn a_payload_with_no_search_is_not_a_view() {
        assert!(ReductionView::from_report(&json!({ "rounds": 0 })).is_none());
        assert!(ReductionView::from_report(&json!({ "search": "not a list" })).is_none());
    }

    /// **The flat log is grouped back into rounds, and nothing is lost doing it.**
    #[test]
    fn the_records_after_a_round_belong_to_it() {
        let v = report(
            json!([
                round(1, 8),
                { "kind": "candidates", "lane": "direct", "group": "direct_admissible", "discovered": 12 },
                { "kind": "attempt", "lane": "direct",
                  "identity": { "form": "direct", "declared_at": { "file": "a.mo", "line": 15 } },
                  "outcome": { "kind": "reduced", "residue": 6 } },
                { "kind": "selected", "lane": "direct",
                  "identity": { "form": "direct", "declared_at": null },
                  "residue_before": 8, "residue_after": 6 },
                { "kind": "stopped", "outcome": { "kind": "sorted" } },
            ]),
            json!({ "n_states": 5, "n_algebraics": 92, "n_equations": 97,
                    "n_manifold_constraints": 2 }),
        );
        let view = ReductionView::from_report(&v).expect("a view");
        assert!(view.problems().is_empty(), "{:?}", view.problems());
        assert_eq!(view.n_rounds(), 1);
        assert_eq!(view.n_discovered(), 12);

        let r = &view.rounds[0];
        assert_eq!(r.number, 1);
        assert_eq!(r.residue, 8);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.attempts.len(), 1);
        assert_eq!(r.attempts[0].at.as_deref(), Some("a.mo:15"));
        assert_eq!(r.attempts[0].residue, Some(6));
        assert_eq!(
            r.selection.as_ref().expect("a selection").residue_after,
            Some(6)
        );
        // The unknown carries Rumoca's own kind beside its name.
        assert_eq!(r.unmatched_unknowns, vec!["emf.w (derivative)".to_owned()]);
    }

    /// **A record arriving before any round is reported, not attached to one.**
    ///
    /// The must-fire half of the grouping: silently hanging an orphan attempt on
    /// whichever round came later would put it in a lane iteration it did not happen
    /// in, and the picture would stay well-formed.
    #[test]
    fn a_record_before_any_round_is_announced() {
        let v = report(
            json!([
                { "kind": "candidates", "lane": "direct", "group": "holonomic", "discovered": 1 },
                round(1, 2),
            ]),
            Value::Null,
        );
        let view = ReductionView::from_report(&v).expect("a view");
        assert_eq!(view.problems().len(), 1, "{:?}", view.problems());
        assert!(
            view.problems()[0].contains("before any round"),
            "{:?}",
            view.problems(),
        );
        assert_eq!(
            view.n_discovered(),
            0,
            "the orphan is not counted into a round"
        );
    }

    /// An unknown record kind is reported rather than skipped.
    ///
    /// Upstream adding a sixth `ReductionRecord` variant is the live case: the pane
    /// cannot render it, and saying so is what stops a shorter search reading as the
    /// whole one.
    #[test]
    fn an_unknown_record_kind_is_announced() {
        let v = report(json!([round(1, 2), { "kind": "teleported" }]), Value::Null);
        let view = ReductionView::from_report(&v).expect("a view");
        assert_eq!(view.problems().len(), 1, "{:?}", view.problems());
        assert!(view.problems()[0].contains("teleported"));
    }

    /// A record with no `kind` at all is counted by `parse_list`.
    #[test]
    fn a_record_with_no_kind_is_counted_rather_than_dropped() {
        let v = report(json!([round(1, 2), { "lane": "direct" }]), Value::Null);
        let view = ReductionView::from_report(&v).expect("a view");
        assert!(!view.problems().is_empty(), "the loss must be stated");
    }

    /// **`CapacitorLoop`'s shape: every count unchanged, and the system changed.**
    #[test]
    fn an_unchanged_size_with_a_manifold_constraint_is_a_change() {
        let mut v = report(json!([round(1, 2)]), Value::Null);
        v["before"] = json!({ "n_states": 1, "n_algebraics": 13, "n_equations": 14 });
        v["after"] = json!({ "n_states": 1, "n_algebraics": 13, "n_equations": 14,
                             "n_manifold_constraints": 1 });
        let view = ReductionView::from_report(&v).expect("a view");
        assert!(
            view.changed_without_resizing(),
            "identical counts plus a manifold constraint is the case the pane must not \
             render as 'nothing happened'",
        );
    }

    /// And the same counts with **no** manifold constraint is not that case.
    ///
    /// The must-fire half: without it, `changed_without_resizing` could return `true`
    /// whenever the sizes match and the sentence would appear on a system reduction
    /// genuinely left alone.
    #[test]
    fn an_unchanged_size_with_no_manifold_constraint_is_not_a_change() {
        let mut v = report(json!([round(1, 2)]), Value::Null);
        v["before"] = json!({ "n_states": 2, "n_algebraics": 0, "n_equations": 2 });
        v["after"] = json!({ "n_states": 2, "n_algebraics": 0, "n_equations": 2,
                             "n_manifold_constraints": 0 });
        let view = ReductionView::from_report(&v).expect("a view");
        assert!(!view.changed_without_resizing());
    }

    /// A system that was not admitted has no `after` side, and does not borrow the
    /// `before` one.
    #[test]
    fn a_refused_system_shows_no_reduced_side() {
        let v = report(json!([round(1, 2)]), Value::Null);
        let view = ReductionView::from_report(&v).expect("a view");
        assert!(view.after.is_none());
        assert!(view.before.is_some(), "the raw system is still sized");
        assert!(!view.changed_without_resizing());
    }

    /// The `before` side carries no manifold column, because the raw system has none.
    ///
    /// `Some(0)` would be a measurement of something that does not apply; the pane
    /// shows an em dash for it.
    #[test]
    fn the_raw_system_has_no_manifold_count() {
        let v = report(json!([round(1, 2)]), Value::Null);
        let view = ReductionView::from_report(&v).expect("a view");
        assert_eq!(view.before.expect("sized").manifold, None);
    }

    /// **The panel reads a real compile, with nothing it could not place.**
    ///
    /// Every test above builds its own payload, so every one of them would still pass
    /// if `worker::index_reduction_from_observation` published a different shape
    /// tomorrow — they agree with each other and with nothing else. This is the only
    /// check here that compares the reader against the writer.
    ///
    /// **`problems` empty is the assertion**, not merely that a view was produced: the
    /// parser is built to carry on past what it cannot read, so a silent shape change
    /// gives a view that renders and omits things. Three specimens, because they cover
    /// different arms — both lanes and a `held` attempt, a refusal with no `after`, and
    /// a run of exactly one record.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn the_panel_reads_a_real_compile() {
        let mut checked = 0usize;
        let mut rounds_seen = 0usize;
        let mut excerpts_seen = 0usize;
        for name in ["Drivetrain", "CartesianPendulum", "SingleInertia"] {
            let crate::worker::FromWorker::Compiled { stages, .. } =
                crate::worker::test_msl::compile_specimen_shared(name)
            else {
                panic!("expected Compiled for {name}");
            };
            let report = stages
                .index_reduction
                .value
                .as_ref()
                .unwrap_or_else(|| panic!("{name} reaches index reduction"));
            let view = ReductionView::from_report(report)
                .unwrap_or_else(|| panic!("{name}: the panel must read a real payload"));
            assert!(
                view.problems().is_empty(),
                "{name}: the panel could not place part of the real payload: {:?}",
                view.problems(),
            );
            // The payload's own two notes must arrive, or the pane shows a residue with
            // no definition and ordinals with no warning.
            assert!(
                view.residue_definition.is_some(),
                "{name}: residue definition"
            );
            assert!(view.ordinals_note.is_some(), "{name}: ordinals note");
            assert_eq!(
                view.n_rounds(),
                report["rounds"].as_u64().unwrap_or(0) as usize,
                "{name}: the grouping must find every round the payload counted",
            );
            rounds_seen += view.n_rounds();
            excerpts_seen += view
                .rounds
                .iter()
                .filter(|r| r.selection.as_ref().is_some_and(|s| s.excerpt.is_some()))
                .count();
            checked += 1;
        }
        assert_eq!(checked, 3);
        // **The lab quotes one of these**, so a real compile has to produce them: the
        // Station 3 expectation is a line of MSL source, not a file name.
        assert!(
            excerpts_seen > 0,
            "no selection on any specimen carried its constraint's source text, so \
             `index-reduction.md` Station 3's expectation is unsatisfiable",
        );
        // Non-vacuity: all three specimens reporting zero rounds would satisfy the
        // round-count equality above while proving nothing about the grouping.
        assert!(
            rounds_seen > 0,
            "no specimen produced a round, so the grouping was never exercised",
        );
    }

    /// **A selection carries the constraint's own source text, not just its file.**
    ///
    /// `docs/fixture-labs/index-reduction.md` Station 3 asks the reader to predict which
    /// *file* the compiler will name and expects to see `phi = flange.phi -
    /// internalSupport.phi` from the MSL's `RotationalEMF` — so the excerpt is a load-
    /// bearing part of a station's `**Expected:**`, not decoration.
    ///
    /// **The station found this missing.** The panel rendered a location on each attempt
    /// and nothing at all on the selection, so that expectation was unsatisfiable when
    /// it was written. Which is the testing the labs buy: composing one exercises the
    /// pane's surface in a way no assertion in this file had.
    #[test]
    fn a_selection_carries_its_constraint_text() {
        let v = report(
            json!([
                round(1, 2),
                { "kind": "selected", "lane": "direct",
                  "identity": { "form": "direct", "declared_at": {
                      "file": "hrw/vendor/msl/Modelica 4.1.0/Electrical/Analog/Basic/RotationalEMF.mo",
                      "line": 41,
                      "excerpt": "phi = flange.phi - internalSupport.phi" } },
                  "residue_before": 2, "residue_after": null },
            ]),
            Value::Null,
        );
        let view = ReductionView::from_report(&v).expect("a view");
        let selection = view.rounds[0].selection.as_ref().expect("a selection");
        assert_eq!(
            selection.at.as_deref(),
            Some("hrw/vendor/msl/Modelica 4.1.0/Electrical/Analog/Basic/RotationalEMF.mo:41"),
        );
        assert_eq!(
            selection.excerpt.as_deref(),
            Some("phi = flange.phi - internalSupport.phi"),
        );
    }

    /// An excerpt that is absent, or whitespace only, is `None` rather than a blank line.
    ///
    /// A blank monospace line under a selection reads as source text that happens to be
    /// empty, which is a claim about the constraint rather than about the span.
    #[test]
    fn a_missing_excerpt_is_absent_rather_than_blank() {
        assert_eq!(excerpt_of(&Value::Null), None);
        assert_eq!(excerpt_of(&json!({ "file": "a.mo", "line": 1 })), None);
        assert_eq!(excerpt_of(&json!({ "excerpt": "   " })), None);
        assert_eq!(
            excerpt_of(&json!({ "excerpt": "  v = p.v - n.v  " })).as_deref(),
            Some("v = p.v - n.v"),
            "trimmed, because a span's slice carries the line's indentation",
        );
    }

    /// **A round number repeats, so the panel must not key anything on it.**
    ///
    /// Rumoca numbers rounds within a lane pass and restarts the counter when a lane is
    /// re-entered, so `CartesianPendulum`'s three records all read round 1. This pins
    /// that the grouping keeps them as three distinct rounds — the regression guard for
    /// an `id_salt` of `(number, lane)`, which gave the first and third the **same egui
    /// id** and one collapsing header driving two sections.
    ///
    /// It cannot assert the id itself: `egui_kittest` sees the accessibility tree and a
    /// collision there is two nodes sharing state, not a missing node. What is checkable
    /// is the shape that makes a collision possible, which is this.
    #[test]
    fn three_rounds_numbered_one_stay_three_rounds() {
        let v = report(
            json!([
                round(1, 2),
                { "kind": "round", "lane": "holonomic", "round": 1, "residue": 2,
                  "unmatched_equations": [], "unmatched_unknowns": [] },
                round(1, 2),
            ]),
            Value::Null,
        );
        let view = ReductionView::from_report(&v).expect("a view");
        assert!(view.problems().is_empty(), "{:?}", view.problems());
        assert_eq!(
            view.n_rounds(),
            3,
            "the pendulum's three passes are three rounds"
        );
        let numbers: Vec<u64> = view.rounds.iter().map(|r| r.number).collect();
        assert_eq!(numbers, vec![1, 1, 1], "and all three are numbered 1");
        let lanes: Vec<&str> = view.rounds.iter().map(|r| r.lane.as_str()).collect();
        assert_eq!(
            lanes,
            vec!["direct", "holonomic", "direct"],
            "so lane does not disambiguate them either \u{2014} only the position does",
        );
    }

    /// A `declared_at` that refused to resolve stays `None`.
    #[test]
    fn an_unresolved_location_is_absent_rather_than_empty() {
        assert_eq!(location_of(&Value::Null), None);
        assert_eq!(location_of(&json!({ "line": 3 })), None);
        assert_eq!(
            location_of(&json!({ "file": "b.mo" })).as_deref(),
            Some("b.mo"),
            "a file with no line still locates something",
        );
    }
}
