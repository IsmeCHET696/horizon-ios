//! The screen a phone shows when it has no game files, and the wait for someone to bring them.
//!
//! `Loading` usually ends one of two ways: a game, or an exit. On a phone the second is worse than
//! the first sounds — the app vanishes and nothing says why, and there is no terminal in the
//! sandbox to read the log in. So a phone that found nothing to load keeps its window, says what it
//! is missing and how to send it, and starts the loader again the moment the files arrive. That
//! restart is the same one a resource-pack change already makes (`restart_menu_resources`), which is
//! why nothing here needs new machinery in the loader itself.
//!
//! Everything drawn is drawn from `horizon_client::import::screen`, with the one atlas that exists
//! before Mojang's files do.
use super::*;

/// The wait: the listener that takes an archive, the atlas the screen draws with, and the words it
/// says.
pub(super) struct Import {
    /// `None` when the port could not be opened, which leaves the cable route and the folder path
    /// on the screen. The screen never promises a page nobody is serving.
    session: Option<horizon_client::import::http::Session>,
    atlas: HudAtlas,
    /// Where an archive has to arrive: the folder the Files app shows.
    home: std::path::PathBuf,
    /// The loader's own words about why there is nothing to show.
    reason: String,
    /// The last upload that was refused, and why.
    error: Option<String>,
    received: u64,
    total: u64,
}

impl Loading {
    /// Stop waiting for a game that cannot start and wait for files instead. The caller has already
    /// said on screen that this is what a phone does (`import::waits_on_screen`).
    pub(super) fn wait_for_files(&mut self, reason: String) {
        let home = horizon_client::import::home();
        let atlas = match HudAtlas::without_pack(&home) {
            Ok(atlas) => atlas,
            Err(error) => {
                // Without even the synthetic atlas there is nothing to draw with, and an exit with
                // the reason in the log is all that is left.
                log::error!("Horizon: the import screen has no font to draw with: {error}");
                self.error = Some(error);
                return;
            }
        };
        horizon_client::ui_theme::reset_atlas();
        let session = match horizon_client::import::http::Session::start(&home) {
            Ok(session) => Some(session),
            Err(error) => {
                log::warn!("Horizon: no upload port: {error}");
                None
            }
        };
        log::warn!(
            "Horizon: no game files to load ({reason}); waiting for an import in {}",
            home.display()
        );
        if let Some(session) = &session {
            // The screen shows this, and the log is the only other place it can be read: a phone
            // plugged into a Mac gets its Documents folder, and a headless run has no screen at all.
            log::info!(
                "Horizon: post an archive to http://{}:{}/upload with the code {}",
                session.address().unwrap_or("this device"),
                session.port(),
                session.pin()
            );
        }
        self.import = Some(Import {
            session,
            atlas,
            home,
            reason,
            error: None,
            received: 0,
            total: 0,
        });
    }

    /// Whether this window is waiting for files, which is when the loader must be left alone: its
    /// channel is closed, and reading it would look like a new failure every frame.
    pub(super) fn importing(&self) -> bool {
        self.import.is_some()
    }

    /// Take whatever the listener has said. An archive that turned into game files ends the wait.
    pub(super) fn poll_import(&mut self) -> Result<()> {
        let Some(import) = &mut self.import else {
            return Ok(());
        };
        let mut ready = false;
        if let Some(session) = &mut import.session {
            for news in session.poll() {
                match news {
                    horizon_client::import::http::News::Progress { received, total } => {
                        import.received = received;
                        import.total = total;
                    }
                    horizon_client::import::http::News::Installed(placed) => {
                        log::info!(
                            "Horizon: imported {} ({} files) to {}",
                            placed.kind.label(),
                            placed.files,
                            placed.path.display()
                        );
                        // A world or a pack is in place already; the title screen lists it as soon
                        // as there is a title screen. The game files are the only thing that can
                        // start the game.
                        ready |= placed.kind == horizon_client::import::Kind::Resources;
                    }
                    horizon_client::import::http::News::Refused { error } => {
                        import.error = Some(error);
                    }
                }
            }
        }
        if ready {
            // Dropping the listener closes the port before the game starts looking at anything.
            let home = self
                .import
                .take()
                .map(|import| import.home)
                .unwrap_or_default();
            return self.load_the_files_that_arrived(home);
        }
        Ok(())
    }

    /// Start the loader again on the files that arrived, as if the first attempt had found them.
    fn load_the_files_that_arrived(&mut self, home: std::path::PathBuf) -> Result<()> {
        self.loaded = None;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.progress = Progress::new();
        let mut options = self.options.take().ok_or("loading already finished")?;
        if let Some(root) = horizon_client::import::resources_root(&options.resources, Some(&home))
        {
            options.resources = root;
        }
        let (receiver, handle) = startup_menu::spawn_loader(
            &options,
            &self.settings,
            &self.vanilla_root,
            self.started,
            &self.progress,
        )?;
        self.receiver = receiver;
        self.handle = Some(handle);
        self.options = Some(options);
        log::info!("Horizon: loading the imported game files");
        Ok(())
    }

    /// One frame of the wait.
    pub(super) fn draw_import(&self, batch: &mut HudBatch) {
        let Some(import) = &self.import else { return };
        let address = import
            .session
            .as_ref()
            .and_then(|session| session.address().map(|host| (host, session.port())));
        horizon_client::import::screen::draw(
            batch,
            &import.atlas,
            &horizon_client::import::screen::Status {
                version: horizon_core::VANILLA_VERSION,
                reason: Some(&import.reason),
                folder: &import.home,
                address,
                pin: import.session.as_ref().map(|session| session.pin()),
                received: import.received,
                total: import.total,
                error: import.error.as_deref(),
            },
        );
    }
}
