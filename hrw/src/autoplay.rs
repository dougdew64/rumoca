//! **Lab structure** — splitting a lab or an Answer into its stations, and naming them.
//!
//! # What this module used to be, and why most of it is gone
//!
//! It was the schedule and clock behind a Play button that ran a lab by itself.
//! `DECISIONS.md`, 2026-08-03: a LinkedIn screenshot of HRW drew interest, explaining
//! *what a lab is* in prose turned out to be harder than showing one, so a lab needed to
//! run itself for long enough to be screen-recorded. Every design decision in it served a
//! recording — a beat was a link rather than a station so something was always moving,
//! time was weighted by prose length, and the run lengths were, in that entry's own
//! words, *"conventional social-video lengths rather than a measured optimum"*.
//!
//! **Doug removed it on 2026-11-05, and the reason is worth keeping**: HRW exists to help
//! him learn, and one of the main ways it does that is as the platform for Claude's
//! answers. The recording feature had begun imposing authoring rules on those answers —
//! a stray `hrw://lab/` link mid-document would make a run abandon the Answer and narrate
//! another file — so a feature with no educational purpose was taxing the one artifact
//! that has nothing but. *"I am concerned about such authoring guidance interfering with
//! your ability to provide good answers."* The full account is in `DECISIONS.md`.
//!
//! **The animation players are a different thing and were not touched.** `playback.rs`
//! steps the frames recorded during a real compile — matching, Tarjan, reduction — and is
//! the *replay* tier of the three-tier progression (`docs/vision.md`). `CLAUDE.md`'s rule
//! that "replay means three things and only one is forbidden" exists precisely so a later
//! session reading this file does not conclude that frame playback went with the clock.
//!
//! # What remains, and who needs it
//!
//! Splitting a document into stations and slugifying their headings, which serve:
//!
//! - **`hrw://lab/<name>/station/<slug>`** — a citation that lands at a station rather
//!   than at the top of a lab.
//! - **`CATALOGUE.md`** — the generated index Claude reads to answer from the fixtures.
//! - **The lab panel's station list**, and `doc_citations`' link checker.
//!
//! None of those needs a link roster or a prose-length measurement, so `LabLink`,
//! `prose_chars` and `links_in_order` went with the clock. A station is now its heading
//! and where that heading starts.

/// One station of a lab: a `##` heading, and where it begins.
///
/// The preamble — everything before the first `##` — is a station too, titled from the
/// document's `#` heading. That is deliberate rather than accidental: `CATALOGUE.md`
/// lists a lab's title among its stations, and a `station/<slug>` link can address it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabStation {
    /// The heading text, with its `#` markers and surrounding space removed.
    pub heading: String,
    /// Byte offset of the heading's line start, **in the text it was parsed from**.
    ///
    /// The pane splits the document here and lets egui compute the scroll position from
    /// the resulting cursor — see `LabState::render_split`, which also carries why a byte
    /// offset cannot be turned into a pixel position by arithmetic.
    ///
    /// Offsets are measured against the **stripped** text (`lab::strip_html_comments`),
    /// which is what the pane renders. `lab::tests_comment_stripping::offsets_are_measured_against_the_stripped_text`
    /// pins that, because stripping at render time would shift every one of them.
    pub heading_offset: usize,
}

/// `## Station 2 — The surprise` becomes `station-2-the-surprise`. Lowercased, with every
/// run of non-alphanumerics collapsed to a single `-` and the ends trimmed.
///
/// **This is what makes `hrw://lab/<name>/station/<slug>` stable.** An ordinal would
/// have been easier and is the wrong choice: inserting a station shifts every later
/// citation **silently**, which is exactly how the `worker.rs:3434` citation in
/// `docs/tech-debt.md` rotted inside a day. A slug is immune to insertion and **fails
/// loudly** when a heading is renamed — and loud failure is the only kind
/// `fixture_lab_links_all_resolve` can act on.
///
/// Deliberately derived rather than authored. An explicit `<!-- anchor: … -->` per
/// station would be more stable still and would put the burden on the lab author for
/// every station, including the ones nothing ever links to. Renames are rare; the
/// checker catches them.
pub fn station_slug(heading: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in heading.trim_start_matches('#').trim().chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    out
}

/// Split lab markdown into stations.
///
/// A `##` heading starts a station; everything before the first one becomes the
/// **preamble station**, titled from the `#` heading.
///
/// Fenced code blocks are skipped when looking for headings, so a `##` comment
/// inside a Modelica or shell block cannot invent a station.
pub fn parse_stations(text: &str) -> Vec<LabStation> {
    let mut stops: Vec<LabStation> = Vec::new();
    let mut title = String::from("Lab");
    let mut current: Option<LabStation> = None;
    let mut in_fence = false;

    // Byte offsets of line starts. Always char boundaries, so the consumer can
    // split the string here without checking.
    let mut seen_bytes = 0usize;

    for line in text.lines() {
        let line_start = seen_bytes;
        seen_bytes += line.len() + 1; // + the newline
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence
            && let Some(rest) = trimmed.strip_prefix("# ")
            && current.is_none()
            && stops.is_empty()
        {
            title = rest.trim().to_owned();
        }
        if !in_fence && let Some(rest) = trimmed.strip_prefix("## ") {
            if let Some(done) = current.take() {
                stops.push(done);
            }
            current = Some(LabStation {
                heading: rest.trim().to_owned(),
                heading_offset: line_start,
            });
            continue;
        }
        if current.is_none() {
            // Still in the preamble; open it lazily so a lab that starts with `##`
            // does not get an empty leading card.
            current = Some(LabStation {
                heading: title.clone(),
                heading_offset: 0,
            });
        }
    }
    if let Some(done) = current.take() {
        stops.push(done);
    }
    stops
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Fixture lab — demo

Preamble prose.

## Station 1 — first

More prose on a second line to make the difference unmistakable.

[a](hrw://load/M/Dae)
[b](hrw://stage/Dae/node/x)

## Station 2 — second

Short.

[c](hrw://notebook/n.nb)
";

    #[test]
    fn parsing_finds_the_preamble_and_every_station_in_order() {
        let stops = parse_stations(SAMPLE);
        assert_eq!(stops.len(), 3, "preamble plus two stops: {stops:#?}");
        assert_eq!(
            stops[0].heading, "Fixture lab — demo",
            "the preamble is titled by `#`"
        );
        assert_eq!(stops[1].heading, "Station 1 — first");
        assert_eq!(stops[2].heading, "Station 2 — second");
    }

    /// Each station's offset must land on its own heading, because the pane slices the
    /// document there. An off-by-one line here sends a `station/<slug>` citation to the
    /// paragraph above or below the station it names.
    #[test]
    fn a_stations_offset_lands_on_its_own_heading() {
        for stop in parse_stations(SAMPLE).iter().skip(1) {
            assert!(
                SAMPLE[stop.heading_offset..].starts_with("## "),
                "{:?} points at {:?}",
                stop.heading,
                &SAMPLE[stop.heading_offset..stop.heading_offset + 20],
            );
        }
    }

    /// A `##` inside a fenced block is code, not a station.
    #[test]
    fn a_heading_inside_a_code_fence_does_not_invent_a_station() {
        let stops = parse_stations("## Real\n```sh\n## not a heading\n```\ntext\n");
        assert_eq!(stops.len(), 1, "one real stop: {stops:#?}");
    }

    /// Slugs are what citations are written against, so the shape is pinned rather than
    /// left to whatever the loop happens to produce.
    #[test]
    fn a_slug_is_lowercase_with_single_dashes_and_no_edges() {
        assert_eq!(
            station_slug("## Station 2 — The surprise"),
            "station-2-the-surprise"
        );
        assert_eq!(station_slug("  Trailing — "), "trailing");
    }
}
