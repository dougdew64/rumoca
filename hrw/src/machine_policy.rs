//! Does this machine stay awake long enough to finish the work?
//!
//! **A sleeping machine fails the same way a missing permission allowlist does:
//! indistinguishable from a hang.** That is why this lives beside the other
//! machine-check policy rather than in a note somebody has to remember. On
//! 2026-08-23 a gate step was recorded at **27,668 s** and diagnosed — by Claude —
//! as build contention; Doug supplied the real cause, which was that the machine
//! had slept. `CLAUDE.md` already carried a **10,780 s** gate run from 2026-08-21
//! "with no source change between them", almost certainly the same thing, and it
//! had stood unexplained for two days.
//!
//! **The clock is what sleep corrupts, never the verdict.** A suite that passes
//! across a suspend still passed. So the damage is not a wrong test result — it is
//! that every timing this repository records becomes untrustworthy without saying
//! so, and timings are what several of its decisions rest on.
//!
//! The parsing lives here rather than in `examples/check_machine.rs` for the reason
//! [`crate::gate_policy`] does: the documented gate runs `--lib` and `--test
//! msl_resolve`, so a test inside an example would never run.

/// `powercfg`'s GUID for *Sleep after*.
///
/// **Matched by GUID, never by the parenthesised English label.** `powercfg`
/// localises those labels, so `(Sleep after)` is `(Standbymodus nach)` on a German
/// Windows and the parser would silently find nothing — which is the failure this
/// module exists to prevent. It is also the repository's standing rule that no
/// substring search decides identity (`docs/identity-and-provenance.md`).
pub const SLEEP_AFTER: &str = "29f6c1db-86da-48c5-9fdb-f2b67b1f44da";

/// `powercfg`'s GUID for *Hibernate after*.
///
/// Checked beside sleep because either one suspends the machine, and a scheme with
/// `standby-timeout-ac 0` but a live hibernate timeout is a real configuration —
/// disabling sleep alone reads as "I turned it off" and is not.
pub const HIBERNATE_AFTER: &str = "9d7815a6-7ee4-497e-8888-515a05f02364";

/// One power setting's two timeouts, in seconds.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Timeout {
    /// Plugged in.
    pub ac_secs: u32,
    /// On battery.
    pub dc_secs: u32,
}

/// Is this timeout value one of `powercfg`'s two spellings of "never"?
///
/// **`0` is the documented one; `0x7fffffff` is the one that bites.** This machine
/// reports hibernate-on-battery as `0x7fffffff`, and read literally that is a
/// 68-year timeout — a naive `!= 0` test would warn about it forever, which trains
/// the reader to ignore the check.
fn never(secs: u32) -> bool {
    secs == 0 || secs == 0x7fff_ffff
}

/// What the machine will do to a long-running job.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SleepRuling {
    /// `powercfg` produced nothing this parser recognised.
    ///
    /// **This is deliberately not a pass.** A parser that reads "fine" out of
    /// output it did not understand is the silent-observer failure this repository
    /// has seven recorded instances of, and the must-fire rule names it directly.
    Unreadable,
    /// The machine suspends while plugged in — blocking.
    SuspendsOnAc {
        /// `"sleep"` or `"hibernate"`, so the fix names the right knob.
        what: &'static str,
        /// The timeout that will fire.
        after_secs: u32,
    },
    /// Safe plugged in; suspends on battery — advisory.
    SuspendsOnBatteryOnly {
        /// The battery timeout that will fire.
        after_secs: u32,
    },
    /// Never suspends on either supply.
    Never,
}

/// Read one setting's AC/DC pair out of a `powercfg /q` dump.
///
/// Returns `None` when the GUID is absent or its indices are unparseable, so a
/// caller cannot mistake "not found" for "zero".
pub fn parse_timeout(dump: &str, guid: &str) -> Option<Timeout> {
    let mut in_setting = false;
    let (mut ac, mut dc) = (None, None);

    for line in dump.lines() {
        let t = line.trim();
        if t.contains("Power Setting GUID:") {
            // A new setting begins; we are inside ours only while this GUID matches.
            in_setting = t.contains(guid);
            continue;
        }
        if !in_setting {
            continue;
        }
        if let Some(v) = t.strip_prefix("Current AC Power Setting Index:") {
            ac = parse_index(v);
        } else if let Some(v) = t.strip_prefix("Current DC Power Setting Index:") {
            dc = parse_index(v);
        }
    }

    Some(Timeout {
        ac_secs: ac?,
        dc_secs: dc?,
    })
}

/// `0x000000b4` → `180`. Decimal is accepted too, since `powercfg` has printed both.
fn parse_index(raw: &str) -> Option<u32> {
    let s = raw.trim();
    match s.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

/// Rule on a `powercfg /q SCHEME_CURRENT SUB_SLEEP` dump.
///
/// **AC blocks and battery only warns**, because the scenario being protected is a
/// multi-hour gate or an unattended run, and those happen plugged in. Refusing to
/// work because a laptop on battery would nap is a check that cries wolf.
pub fn rule(dump: &str) -> SleepRuling {
    let sleep = parse_timeout(dump, SLEEP_AFTER);
    let hibernate = parse_timeout(dump, HIBERNATE_AFTER);

    // Neither setting present means we are not reading powercfg output at all.
    let (Some(sleep), Some(hibernate)) = (sleep, hibernate) else {
        return SleepRuling::Unreadable;
    };

    if !never(sleep.ac_secs) {
        return SleepRuling::SuspendsOnAc {
            what: "sleep",
            after_secs: sleep.ac_secs,
        };
    }
    if !never(hibernate.ac_secs) {
        return SleepRuling::SuspendsOnAc {
            what: "hibernate",
            after_secs: hibernate.ac_secs,
        };
    }
    if !never(sleep.dc_secs) {
        return SleepRuling::SuspendsOnBatteryOnly {
            after_secs: sleep.dc_secs,
        };
    }
    SleepRuling::Never
}

/// The command that fixes what `rule` objected to.
pub fn fix_for(ruling: SleepRuling) -> Option<String> {
    match ruling {
        SleepRuling::SuspendsOnAc { what, .. } => {
            let knob = if what == "sleep" {
                "standby-timeout-ac"
            } else {
                "hibernate-timeout-ac"
            };
            Some(format!("powercfg /change {knob} 0"))
        }
        SleepRuling::SuspendsOnBatteryOnly { .. } => {
            Some("powercfg /change standby-timeout-dc 0   (only if you run unplugged)".into())
        }
        SleepRuling::Unreadable | SleepRuling::Never => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Doug's machine on 2026-08-25, captured verbatim from `powercfg`.
    const REAL_DUMP: &str = "\
Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)
  Subgroup GUID: 238c9fa8-0aad-41ed-83f4-97be242c8f20  (Sleep)
    Power Setting GUID: 29f6c1db-86da-48c5-9fdb-f2b67b1f44da  (Sleep after)
      Minimum Possible Setting: 0x00000000
      Current AC Power Setting Index: 0x00000000
      Current DC Power Setting Index: 0x000000b4
    Power Setting GUID: 94ac6d29-73ce-41a6-809f-6363ba21b47e  (Allow hybrid sleep)
      Current AC Power Setting Index: 0x00000001
      Current DC Power Setting Index: 0x00000001
    Power Setting GUID: 9d7815a6-7ee4-497e-8888-515a05f02364  (Hibernate after)
      Current AC Power Setting Index: 0x00000000
      Current DC Power Setting Index: 0x7fffffff
";

    #[test]
    fn a_real_powercfg_dump_reads_as_safe_on_ac_and_napping_on_battery() {
        assert_eq!(
            parse_timeout(REAL_DUMP, SLEEP_AFTER),
            Some(Timeout {
                ac_secs: 0,
                dc_secs: 180
            })
        );
        assert_eq!(
            rule(REAL_DUMP),
            SleepRuling::SuspendsOnBatteryOnly { after_secs: 180 }
        );
    }

    /// The indices of a *neighbouring* setting must not be attributed to ours.
    ///
    /// `Allow hybrid sleep` sits between the two settings we read and carries index
    /// `0x1` on both supplies. A scan that forgot to close the previous GUID would
    /// report sleep-after as 1 second and block a perfectly configured machine.
    #[test]
    fn a_neighbouring_settings_indices_are_not_attributed_to_this_one() {
        assert_eq!(
            parse_timeout(REAL_DUMP, HIBERNATE_AFTER),
            Some(Timeout {
                ac_secs: 0,
                dc_secs: 0x7fff_ffff
            })
        );
    }

    #[test]
    fn a_machine_that_sleeps_while_plugged_in_is_blocking() {
        let dump = REAL_DUMP.replace(
            "      Current AC Power Setting Index: 0x00000000\n      Current DC Power Setting Index: 0x000000b4",
            "      Current AC Power Setting Index: 0x00000384\n      Current DC Power Setting Index: 0x000000b4",
        );
        assert_eq!(
            rule(&dump),
            SleepRuling::SuspendsOnAc {
                what: "sleep",
                after_secs: 900
            }
        );
        assert_eq!(
            fix_for(rule(&dump)).as_deref(),
            Some("powercfg /change standby-timeout-ac 0")
        );
    }

    /// Sleep disabled but hibernate live still suspends the machine.
    #[test]
    fn disabling_sleep_alone_does_not_satisfy_the_check() {
        let dump = REAL_DUMP.replace(
            "      Current AC Power Setting Index: 0x00000000\n      Current DC Power Setting Index: 0x7fffffff",
            "      Current AC Power Setting Index: 0x00000e10\n      Current DC Power Setting Index: 0x7fffffff",
        );
        assert_eq!(
            rule(&dump),
            SleepRuling::SuspendsOnAc {
                what: "hibernate",
                after_secs: 3600
            }
        );
        assert_eq!(
            fix_for(rule(&dump)).as_deref(),
            Some("powercfg /change hibernate-timeout-ac 0")
        );
    }

    /// `0x7fffffff` is "never", not a 68-year timeout.
    #[test]
    fn the_never_sentinel_is_not_read_as_a_sixty_eight_year_timeout() {
        assert!(never(0));
        assert!(never(0x7fff_ffff));
        assert!(!never(180));
        let dump = REAL_DUMP.replace(
            "      Current DC Power Setting Index: 0x000000b4",
            "      Current DC Power Setting Index: 0x7fffffff",
        );
        assert_eq!(rule(&dump), SleepRuling::Never);
    }

    /// **The must-fire test.** Output this parser cannot read must never pass.
    ///
    /// Every way `powercfg` can fail to answer lands here: absent from `PATH`,
    /// refusing the arguments (it did exactly that under MSYS, which rewrote `/q`
    /// into a Windows path), a localised build whose GUIDs somehow moved, or a
    /// future scheme that drops the subgroup. All of them produce output with no
    /// recognised GUID, and all of them must be visible rather than green.
    #[test]
    fn output_this_parser_cannot_read_is_never_reported_as_safe() {
        for junk in [
            "",
            "Invalid Parameters -- try \"/?\" for help",
            "Power Scheme GUID: 381b4222-f694-41f0-9685-ff5bb260df2e  (Balanced)",
        ] {
            assert_eq!(
                rule(junk),
                SleepRuling::Unreadable,
                "unreadable output was ruled on as if it had been understood: {junk:?}"
            );
        }
    }

    /// A setting present but truncated is unreadable, not zero.
    #[test]
    fn a_setting_missing_its_dc_index_does_not_default_to_never() {
        let dump = "\
    Power Setting GUID: 29f6c1db-86da-48c5-9fdb-f2b67b1f44da  (Sleep after)
      Current AC Power Setting Index: 0x00000000
    Power Setting GUID: 9d7815a6-7ee4-497e-8888-515a05f02364  (Hibernate after)
      Current AC Power Setting Index: 0x00000000
      Current DC Power Setting Index: 0x00000000
";
        assert_eq!(parse_timeout(dump, SLEEP_AFTER), None);
        assert_eq!(rule(dump), SleepRuling::Unreadable);
    }
}
