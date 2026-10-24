//! Horizon: native Rust client for Minecraft Java Edition 26.3 gameplay.
#![forbid(unsafe_code)]
mod game;

use horizon_client::{crash, logging};

/// Start the game where a phone lets it write.
///
/// iOS hands an app a working directory of `/`, which is read-only, while the game builds its world saves,
/// its `options.txt`, its `.horizon-cache` and its `resourcepacks/` from `current_dir()` -- so every one of
/// those is a path it cannot create. The writable place is the app's data container, and `$HOME` names it,
/// so the fix is one call made before anything reads a path: work in `Documents`. The launcher runs the game
/// inside its game home instead; a phone has no launcher, and `Documents` is the folder the Files app shows,
/// which is where a player drops a resource pack or a world.
///
/// Prepared resources are not looked for here: `Options::read` checks a `resources/` beside the executable
/// before anything else, and on a phone that is `Horizon.app/resources/`, so assets shipped inside the bundle
/// are found with no argument to pass and no new path to invent.
#[cfg(target_os = "ios")]
fn into_app_sandbox() -> Result<(), String> {
    let documents = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| std::path::PathBuf::from(home).join("Documents"))
        .ok_or("iOS gave this app no HOME")?;
    let describe = |error: std::io::Error| format!("{}: {error}", documents.display());
    std::fs::create_dir_all(&documents).map_err(describe)?;
    std::env::set_current_dir(&documents).map_err(describe)?;
    Ok(())
}

fn main() {
    #[cfg(target_os = "ios")]
    if let Err(error) = into_app_sandbox() {
        eprintln!("Horizon: {error}");
        std::process::exit(1);
    }
    horizon_client::telemetry::mark_start();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let game_args = args.iter().take_while(|a| *a != "--dedicated");
    let help = game_args.clone().any(|a| a == "--help" || a == "-h");
    // The dedicated server logs in its own folder, as vanilla's does; the game in its home.
    let (home, side, thread) = if args.iter().any(|a| a == "--dedicated") {
        (std::path::PathBuf::from("."), "server", "Server thread")
    } else {
        (logging::game_home(), "client", "Render thread")
    };
    if !help {
        logging::init(&home, thread);
        crash::install(home.clone(), side);
    }
    if let Err(error) = game::run() {
        log::error!("Horizon: {error}");
        if !help {
            crash::report_error(&home, side, &*error);
        }
        // Started from Explorer, the console window closes with the game:
        // keep the message on screen until it is read.
        #[cfg(windows)]
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            eprintln!("Press Enter to close.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
    logging::flush();
}
