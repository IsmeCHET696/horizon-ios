//! The phone's launcher (packaging/ios/Launcher), answered: it speaks the control socket's language through
//! `horizon_ios_launcher`'s queue, so `worlds`, `open`, `connect`, `options-set` and the rest mean what they
//! mean to horizon-ctl. Three lines are its own: `launcher-state`, which says whether the game is starting,
//! waiting for files or at its title; `import PATH`, which installs an archive the launcher downloaded; and
//! `launcher-world {json}`, Select World's actions for folders with spaces in their names.
//!
//! The loading window answers those two before there is a `Game` to ask; anything else waits for the title.
use super::{Game, Loading};
use horizon_client::{
    Result,
    import::{Installed, Kind, Pack},
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    sync::mpsc::{self, Receiver, TryRecvError},
};
use winit::event_loop::ActiveEventLoop;

/// What came back for a launcher request so far: a control answer, or an import's result.
enum Waiting {
    Control(Receiver<String>),
    Import(Receiver<std::result::Result<Installed, String>>),
}

thread_local! {
    /// Launcher requests whose answers have not come back yet. Only the main thread touches them.
    static PENDING: RefCell<Vec<(Waiting, horizon_ios_launcher::Request)>> = const { RefCell::new(Vec::new()) };
}

/// Answer what has come back and keep waiting on the rest. True when an import brought game files.
fn deliver() -> bool {
    let mut resources = false;
    let pending = PENDING.with_borrow_mut(std::mem::take);
    let mut still = Vec::new();
    for (waiting, request) in pending {
        match waiting {
            Waiting::Control(answers) => match answers.try_recv() {
                Ok(answer) => request.answer(&answer),
                // Dropping the request answers it with an error.
                Err(TryRecvError::Disconnected) => {}
                Err(TryRecvError::Empty) => still.push((Waiting::Control(answers), request)),
            },
            Waiting::Import(result) => match result.try_recv() {
                Ok(Ok(installed)) => {
                    resources |= installed.kind == Kind::Resources;
                    request.answer(&installed_json(&installed).to_string());
                }
                Ok(Err(error)) => request.answer(&json!({ "error": error }).to_string()),
                Err(TryRecvError::Disconnected) => {}
                Err(TryRecvError::Empty) => still.push((Waiting::Import(result), request)),
            },
        }
    }
    PENDING.with_borrow_mut(|pending| pending.extend(still));
    resources
}

/// Install the archive named by `import PATH` on a worker: a prepared tree is hundreds of megabytes.
fn start_import(request: horizon_ios_launcher::Request) {
    let path = request
        .line
        .trim_start()
        .strip_prefix("import")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from);
    let Some(path) = path else {
        request.answer(r#"{"error":"import PATH: the archive to install"}"#);
        return;
    };
    let home = horizon_client::import::home();
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("horizon-import".into())
        .spawn(move || {
            let installed = Pack::open(&path)
                .and_then(|pack| pack.install(&home))
                .map_err(|e| e.to_string());
            let _ = sender.send(installed);
        });
    match spawned {
        Ok(_) => PENDING.with_borrow_mut(|p| p.push((Waiting::Import(receiver), request))),
        Err(error) => request.answer(&json!({ "error": error.to_string() }).to_string()),
    }
}

fn installed_json(installed: &Installed) -> Value {
    json!({
        "ok": true,
        "kind": match installed.kind {
            Kind::Resources => "resources",
            Kind::World => "world",
            Kind::ResourcePack => "resource_pack",
        },
        "name": installed.path.file_name().map(|n| n.to_string_lossy().into_owned()),
        "files": installed.files,
        "bytes": installed.bytes,
    })
}

impl Game<'_> {
    /// The launcher's requests, once a frame: its own lines here, the rest through the control socket's
    /// dispatcher, as if horizon-ctl had sent them.
    pub(super) fn poll_launcher(&mut self, event_loop: &ActiveEventLoop) {
        for request in horizon_ios_launcher::take() {
            let command = request.line.split_whitespace().next().unwrap_or_default();
            match command {
                "launcher-state" => {
                    let state = json!({
                        "phase": if self.title.is_some() { "title" } else { "world" },
                        "screen": self.screen_name(),
                        "player": self.options.player_name(),
                        "version": horizon_core::version::VERSION_ID,
                    });
                    request.answer(&state.to_string());
                }
                "import" => start_import(request),
                "launcher-world" => self.launcher_world(request),
                _ => {
                    let (reply, answers) = mpsc::channel();
                    let line = request.line.clone();
                    self.answer(horizon_client::control::Request { line, reply }, event_loop);
                    PENDING.with_borrow_mut(|p| p.push((Waiting::Control(answers), request)));
                }
            }
        }
        // Game files that arrive while the game runs are read on its next start.
        deliver();
    }

    /// `launcher-world {"action": "open"|"rename"|"backup"|"delete", "folder": ..., "name": ...}`: Select World
    /// and Edit World by folder, for folders the control socket's word-split lines cannot name ("New World").
    fn launcher_world(&mut self, request: horizon_ios_launcher::Request) {
        let answer = |request: horizon_ios_launcher::Request, value: Value| {
            request.answer(&value.to_string());
        };
        let args: Value = request
            .line
            .trim_start()
            .strip_prefix("launcher-world")
            .and_then(|rest| serde_json::from_str(rest).ok())
            .unwrap_or_default();
        let (Some(action), Some(folder)) = (args["action"].as_str(), args["folder"].as_str())
        else {
            answer(
                request,
                json!({ "error": "launcher-world {\"action\", \"folder\"}" }),
            );
            return;
        };
        if self.title.is_none() {
            answer(request, json!({ "error": "a world is already open" }));
            return;
        }
        let saves = std::env::current_dir().unwrap_or_default().join("saves");
        let done = match action {
            "open" => self.open_world(folder, None, None, false, None),
            "rename" => {
                let name = args["name"].as_str().unwrap_or_default().trim();
                if name.is_empty() {
                    Err("a world needs a name".into())
                } else {
                    horizon_client::title::world_management::rename(&saves, folder, name)
                }
            }
            "delete" => horizon_client::title::world_management::delete(&saves, folder),
            "backup" => {
                // A big world takes a while to zip: off the game thread, as Make Backup does.
                let (reply, answers) = mpsc::channel();
                let folder = folder.to_owned();
                std::thread::spawn(move || {
                    let value =
                        match horizon_client::title::world_management::backup(&saves, &folder) {
                            Ok(bytes) => json!({ "ok": true, "bytes": bytes }),
                            Err(e) => json!({ "error": format!("backup of {folder} failed: {e}") }),
                        };
                    let _ = reply.send(value.to_string());
                });
                PENDING.with_borrow_mut(|p| p.push((Waiting::Control(answers), request)));
                return;
            }
            _ => Err(format!("launcher-world: no action {action}").into()),
        };
        if let Some(title) = self.title.as_mut() {
            title.worlds_changed();
        }
        match done {
            Ok(()) => answer(request, json!({ "ok": true })),
            Err(e) => answer(request, json!({ "error": e.to_string() })),
        }
    }
}

impl Loading {
    /// The launcher's requests before the game exists: its state, and imports, which is how a phone with no
    /// game files gets them from the launcher. Game files that arrive end the wait for them.
    pub(super) fn poll_launcher(&mut self) -> Result<()> {
        for request in horizon_ios_launcher::take() {
            let command = request.line.split_whitespace().next().unwrap_or_default();
            match command {
                "launcher-state" => {
                    let state = if self.importing() {
                        json!({ "phase": "needs_files", "reason": self.import_reason() })
                    } else {
                        json!({ "phase": "loading", "progress": self.progress.fraction() })
                    };
                    request.answer(&state.to_string());
                }
                "import" => start_import(request),
                _ => request.answer(
                    &json!({ "error": "the game is still starting", "starting": true }).to_string(),
                ),
            }
        }
        if deliver() && self.importing() {
            self.files_arrived()?;
        }
        Ok(())
    }
}
