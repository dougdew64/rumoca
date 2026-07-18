//! HRW Observatory — an egui instrument for studying the Rumoca Modelica
//! compiler pipeline. See `docs/CHARTER.md` and `CLAUDE.md`.
//!
//! Arc 1: eframe shell, file picker over the specimen directory, generic
//! serde-value tree inspector showing the parsed AST.

mod app;
mod tree;
mod worker;

fn main() -> eframe::Result<()> {
    // WSLg advertises a Wayland display (`WAYLAND_DISPLAY=wayland-0`) but places
    // the socket under `/mnt/wslg/runtime-dir` while leaving `XDG_RUNTIME_DIR`
    // pointing elsewhere, so the path winit resolves does not exist and it dies
    // with `NoCompositor`. winit 0.30 no longer honors `WINIT_UNIX_BACKEND`, and
    // it forbids creating a second event loop, so a post-failure retry is
    // impossible — the choice must be made before `run_native`. We probe the
    // exact socket winit would use; if it is unreachable and X11 is available,
    // we hide the Wayland env so winit selects X11. Where Wayland genuinely
    // works (a real Linux box) the probe succeeds and Wayland is kept.
    #[cfg(unix)]
    prefer_x11_if_wayland_dead();

    run_app()
}

fn run_app() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("HRW Observatory")
            .with_inner_size([1100.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "HRW Observatory",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

/// If a Wayland display is advertised but its socket is unreachable, and an X11
/// display exists, drop the Wayland environment so winit falls back to X11.
#[cfg(unix)]
fn prefer_x11_if_wayland_dead() {
    let x11_available = std::env::var("DISPLAY").map(|v| !v.is_empty()).unwrap_or(false);
    if x11_available && wayland_advertised() && !wayland_socket_reachable() {
        eprintln!("HRW: Wayland socket unreachable; using X11.");
        // SAFETY: called at startup on the main thread, before the worker
        // thread or the event loop exists, so no other thread can observe the
        // environment changing.
        unsafe {
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::remove_var("WAYLAND_SOCKET");
        }
    }
}

#[cfg(unix)]
fn wayland_advertised() -> bool {
    let nonempty = |k| std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false);
    nonempty("WAYLAND_DISPLAY") || nonempty("WAYLAND_SOCKET")
}

/// Can we open the Wayland socket winit would use? Mirrors winit's own path
/// resolution: an absolute `WAYLAND_DISPLAY` is used directly, otherwise it is
/// joined onto `XDG_RUNTIME_DIR`. When the path can't be determined we assume
/// reachable and let winit try.
#[cfg(unix)]
fn wayland_socket_reachable() -> bool {
    use std::os::unix::net::UnixStream;
    use std::path::{Path, PathBuf};

    let disp = match std::env::var("WAYLAND_DISPLAY") {
        Ok(d) if !d.is_empty() => d,
        _ => return false,
    };
    let path = if disp.starts_with('/') {
        PathBuf::from(disp)
    } else {
        match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(dir) => Path::new(&dir).join(disp),
            None => return true, // can't determine; let winit attempt it
        }
    };
    UnixStream::connect(path).is_ok()
}
