//! **Rewrite the generated regions of `docs/architecture.md`.**
//!
//! ```text
//! cargo run -p hrw --example gen_architecture
//! ```
//!
//! **The generator itself lives in `hrw::arch_doc`**, so
//! `arch_doc::tests::architecture_regions_are_current` checks the same code that
//! writes the file rather than a second implementation of it — the same split, for
//! the same reason, as `gen_lab_catalogue` and `hrw::lab::catalogue`.
//!
//! Unlike `CATALOGUE.md`, `architecture.md` is **not** generated whole: it is
//! hand-written reasoning with derived numbers in marker-delimited regions, and
//! this rewrites only what is between the markers. See the module docs for what is
//! generated, what is deliberately left to prose, and the twenty stale counts that
//! prompted it.

fn main() {
    let path = hrw::arch_doc::architecture_path();
    let before = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            std::process::exit(1);
        }
    };

    // A splice failure is a *finding* — a marker was renamed or lost — so it exits
    // non-zero rather than writing a file that silently kept its stale numbers.
    let after = match hrw::arch_doc::splice(&before) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    if after == before {
        println!("{} is already current", path.display());
        return;
    }

    if let Err(e) = std::fs::write(&path, &after) {
        eprintln!("cannot write {}: {e}", path.display());
        std::process::exit(1);
    }

    // Read back what was written. `CLAUDE.md`: "read back anything a shell wrote" —
    // the same courtesy applies to anything a generator wrote, and it costs one
    // syscall.
    match std::fs::read_to_string(&path) {
        Ok(readback) if readback == after => {
            println!("wrote {} ({} bytes)", path.display(), after.len());
        }
        Ok(_) => {
            eprintln!("{} does not match what was written", path.display());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("cannot read back {}: {e}", path.display());
            std::process::exit(1);
        }
    }
}
