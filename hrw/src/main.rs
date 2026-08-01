//! HRW Observatory — entry point.
//!
//! This is the binary crate's `main.rs`. Its only job is to:
//!   1. Work around a WSLg/Wayland bug (see below), then
//!   2. Launch the eframe (egui's native windowing wrapper) event loop.
//!
//! All real application logic lives in the library crate (`lib.rs` / `app.rs`).
//! Separating the binary entry point from the library lets headless tools
//! (like `examples/gen_trace`) reuse the compilation pipeline without pulling
//! in the GUI event loop.
//!
//! **Why eframe?** egui is an immediate-mode GUI library — the app redraws
//! every frame by calling `update()`. eframe provides the native window, GL
//! context, and event loop that drives those `update()` calls. The `App` struct
//! (in `app.rs`) implements `eframe::App`, and eframe owns the main loop.

use hrw::app;

const APP_NAME: &str = "HRW Observatory";

fn main() -> eframe::Result<()> {
    // --- WSLg Wayland workaround ---
    //
    // Background: WSL2's graphical layer (WSLg) sets `WAYLAND_DISPLAY=wayland-0`
    // but places the actual Unix socket under `/mnt/wslg/runtime-dir`, while
    // `XDG_RUNTIME_DIR` points to a *different* directory. When winit (the
    // window-creation library egui uses) resolves the socket path, it joins
    // `WAYLAND_DISPLAY` onto `XDG_RUNTIME_DIR` — and the resulting path does
    // not exist, causing a `NoCompositor` panic.
    //
    // Complication: winit 0.30 removed the `WINIT_UNIX_BACKEND` env var and
    // forbids creating a second event loop, so we cannot try Wayland first and
    // fall back to X11 on failure. The decision must be made *before*
    // `run_native`.
    //
    // Solution: probe the exact socket path winit would use. If it is
    // unreachable and X11 (`DISPLAY`) is available, remove the Wayland env vars
    // so winit selects X11 automatically. On a real Linux desktop where Wayland
    // works, the probe succeeds and Wayland is kept.
    #[cfg(unix)]
    prefer_x11_if_wayland_dead();

    run_app()
}

/// Half the screen width, full screen height — used with `--half` for
/// side-by-side Debug mode with VS Code. Falls back to 960×700 if the screen
/// size can't be queried.
fn initial_window_geometry() -> (eframe::egui::Vec2, Option<eframe::egui::Pos2>) {
    if let Some((w, h)) = query_screen_dimensions() {
        let size = eframe::egui::Vec2::new(w as f32 / 2.0, h as f32 * 0.85);
        let pos = eframe::egui::Pos2::new(
            (w as f32 - size.x) / 2.0,
            (h as f32 - size.y) / 2.0,
        );
        (size, Some(pos))
    } else {
        (eframe::egui::Vec2::new(960.0, 700.0), None)
    }
}

/// Query the X11 display dimensions via `xdpyinfo`. Returns `None` on Wayland-only
/// systems or if the command isn't available.
fn query_screen_dimensions() -> Option<(u32, u32)> {
    let output = std::process::Command::new("xdpyinfo")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("dimensions:") {
            // "3440x1440 pixels (910x381 millimeters)"
            let dims = rest.split_whitespace().next()?;
            let (w, h) = dims.split_once('x')?;
            return Some((w.parse().ok()?, h.parse().ok()?));
        }
    }
    None
}

// Configure the native window and start the eframe event loop.
//
// `NativeOptions` controls the initial window state (title, maximized, etc.).
// The closure passed to `run_native` is a *factory*: eframe calls it once after
// creating the GL context, receiving a `CreationContext` (`cc`) that provides
// access to the egui context for one-time setup (fonts, visuals, persistence).
// The factory returns a boxed `App` that eframe will call `update()` on each
// frame for the lifetime of the application.
fn run_app() -> eframe::Result<()> {
    let half_screen = std::env::args().any(|a| a == "--half");
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title(APP_NAME);
    if half_screen {
        let (size, position) = initial_window_geometry();
        viewport = viewport.with_inner_size(size);
        if let Some(pos) = position {
            viewport = viewport.with_position(pos);
        }
    } else {
        viewport = viewport.with_maximized(true);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        options,
        // The `Box::new(|cc| ...)` is a closure that acts as an app factory.
        // eframe calls it exactly once. `cc` (CreationContext) gives access to
        // the egui context before the first frame — used for setting up fonts,
        // visuals, and spawning the background worker thread.
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

/// If a Wayland display is advertised but its socket is unreachable, and an X11
/// display exists, drop the Wayland environment so winit falls back to X11.
///
/// # Safety
///
/// `std::env::remove_var` is unsafe in Rust 2024 edition because modifying the
/// environment is not thread-safe. We call this at the very start of `main`,
/// before any other thread exists, so it is safe here.
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

// Check whether the environment advertises a Wayland display.
// WSLg always sets `WAYLAND_DISPLAY`; a real Wayland compositor may also set
// `WAYLAND_SOCKET` (an already-opened fd).
#[cfg(unix)]
fn wayland_advertised() -> bool {
    // The closure `nonempty` checks a single env var: present and non-empty.
    let nonempty = |k| std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false);
    nonempty("WAYLAND_DISPLAY") || nonempty("WAYLAND_SOCKET")
}

/// Can we open the Wayland socket winit would use? Mirrors winit's own path
/// resolution: an absolute `WAYLAND_DISPLAY` is used directly, otherwise it is
/// joined onto `XDG_RUNTIME_DIR`. When the path can't be determined we assume
/// reachable and let winit try.
///
/// This uses a Unix domain socket (`UnixStream`) connection attempt as a
/// reachability probe — if `connect` succeeds, Wayland is genuinely available.
#[cfg(unix)]
fn wayland_socket_reachable() -> bool {
    use std::os::unix::net::UnixStream;
    use std::path::{Path, PathBuf};

    // Read `WAYLAND_DISPLAY` — typically "wayland-0" (relative) or an absolute
    // path to the socket.
    let disp = match std::env::var("WAYLAND_DISPLAY") {
        Ok(d) if !d.is_empty() => d,
        _ => return false,
    };
    // Resolve the socket path the same way winit does:
    // - absolute path → use directly
    // - relative name → join onto XDG_RUNTIME_DIR (e.g. "/run/user/1000/wayland-0")
    let path = if disp.starts_with('/') {
        PathBuf::from(disp)
    } else {
        match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(dir) => Path::new(&dir).join(disp),
            None => return true, // can't determine; let winit attempt it
        }
    };
    // Attempt a Unix domain socket connection — the definitive reachability test.
    UnixStream::connect(path).is_ok()
}
