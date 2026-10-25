//! The phone's launcher, in the game's own bundle. The game owns the app (winit runs UIKit's main loop and
//! never hands it back), so the launcher cannot run before it: Xcode embeds the SwiftUI launcher as
//! `Frameworks/HorizonLauncherKit.framework`, and once the game's window is up [`present`] loads it and asks
//! it to cover the game. Its Play button uncovers the game, at its title screen.
//!
//! Loaded at run time rather than linked, so a bundle without the framework (the workflow's hand-assembled
//! one, a desktop build) is the game as before. The game forbids `unsafe`, so the loading lives here.
//!
//! The launcher talks to the game in the game's own control language (docs/horizon-ctl-commands.md): it
//! hands [`present`]'s caller a line such as `worlds` or `open My World`, and the game answers with the JSON
//! the control socket would. The two share one thread, the main one, so nothing here waits: a line is queued
//! ([`take`]), the game answers it on its next frame, and the answer goes back to Swift through the callback
//! that came with it.

use std::{
    collections::VecDeque,
    sync::{Mutex, PoisonError},
};

/// One line from the launcher, and the way back to it.
pub struct Request {
    pub line: String,
    reply: Option<Box<dyn FnOnce(&str) + Send>>,
}

impl Request {
    /// A request answered by `reply`; the launcher's own requests are made by [`present`]'s bridge, and
    /// tests make theirs here.
    pub fn new(line: impl Into<String>, reply: impl FnOnce(&str) + Send + 'static) -> Self {
        Self {
            line: line.into(),
            reply: Some(Box::new(reply)),
        }
    }

    /// Send the JSON answer back to the launcher.
    pub fn answer(mut self, json: &str) {
        if let Some(reply) = self.reply.take() {
            reply(json);
        }
    }
}

impl Drop for Request {
    /// A request nobody answered still answers, so the launcher never waits on a line the game dropped.
    fn drop(&mut self) {
        if let Some(reply) = self.reply.take() {
            reply(r#"{"error":"the game dropped this request"}"#);
        }
    }
}

static QUEUE: Mutex<VecDeque<Request>> = Mutex::new(VecDeque::new());

/// Queue a request for the game's next frame.
pub fn push(request: Request) {
    QUEUE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push_back(request);
}

/// The requests the launcher sent since the last call, oldest first. Empty everywhere but a phone.
pub fn take() -> Vec<Request> {
    QUEUE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .drain(..)
        .collect()
}

/// Load the launcher and show it over the game, once per run: the loading screen's window asks, and so does
/// the game's when it makes its own. False when this build or bundle has none, or it was already shown.
pub fn present() -> bool {
    static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if SHOWN.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    show()
}

/// Show the launcher over the game again (Quit Game, which an iPhone app does not do itself). False when this
/// build or bundle has none; the launcher ignores a second request while it is up.
pub fn show() -> bool {
    #[cfg(target_os = "ios")]
    return ios::present();
    #[cfg(not(target_os = "ios"))]
    false
}

#[cfg(target_os = "ios")]
mod ios {
    use std::ffi::{CStr, CString, c_char, c_void};

    /// `horizon_launcher_present` in packaging/ios/Launcher/Present.swift.
    const SYMBOL: &CStr = c"horizon_launcher_present";

    /// How the launcher hears back: its context, and the answer as NUL-terminated UTF-8 JSON that lives
    /// only for the call.
    type Done = extern "C" fn(ctx: *mut c_void, reply: *const c_char);
    /// How the launcher asks: a NUL-terminated line, and where its answer goes.
    type Submit = extern "C" fn(line: *const c_char, ctx: *mut c_void, done: Done);

    extern "C" fn submit(line: *const c_char, ctx: *mut c_void, done: Done) {
        let line = if line.is_null() {
            String::new()
        } else {
            // SAFETY: the launcher passes a NUL-terminated string it keeps alive for this call
            // (`withCString` in Bridge.swift), and nothing here keeps the pointer past it.
            unsafe { CStr::from_ptr(line) }
                .to_string_lossy()
                .into_owned()
        };
        // The context is the launcher's: carried back untouched, as an address so the reply is Send.
        let ctx = ctx as usize;
        super::push(super::Request::new(line, move |json: &str| {
            let text = CString::new(json.replace('\0', "")).unwrap_or_default();
            done(ctx as *mut c_void, text.as_ptr());
        }));
    }

    pub fn present() -> bool {
        let Some(path) = std::env::current_exe().ok().and_then(|exe| {
            let framework = exe
                .parent()?
                .join("Frameworks/HorizonLauncherKit.framework/HorizonLauncherKit");
            framework
                .exists()
                .then(|| CString::new(framework.into_os_string().into_encoded_bytes()).ok())?
        }) else {
            log::info!("Horizon: no launcher in this bundle; starting at the title screen.");
            return false;
        };
        // SAFETY: dlopen and dlsym on a framework Xcode built and signed into this bundle; the symbol is a
        // `@_cdecl` Swift function taking the bridge's C function pointer and returning a Bool, called on
        // the main thread (the game's window event), which is where UIKit has to be touched.
        unsafe {
            let handle = libc::dlopen(path.as_ptr(), libc::RTLD_NOW);
            if handle.is_null() {
                log::warn!("Horizon: the launcher framework did not load");
                return false;
            }
            let symbol = libc::dlsym(handle, SYMBOL.as_ptr());
            if symbol.is_null() {
                log::warn!("Horizon: the launcher framework has no {SYMBOL:?}");
                return false;
            }
            let present: extern "C" fn(Submit) -> bool = std::mem::transmute(symbol);
            present(submit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn requests_come_out_in_order_and_answer_once() {
        let answers = Arc::new(Mutex::new(Vec::new()));
        for line in ["worlds", "servers"] {
            let answers = answers.clone();
            push(Request::new(line, move |json: &str| {
                answers.lock().unwrap().push(json.to_owned())
            }));
        }
        let taken = take();
        assert_eq!(
            taken.iter().map(|r| r.line.as_str()).collect::<Vec<_>>(),
            ["worlds", "servers"]
        );
        assert!(take().is_empty());
        let mut taken = taken.into_iter();
        taken.next().unwrap().answer(r#"{"ok":true}"#);
        // Dropped unanswered: the launcher still hears back.
        drop(taken);
        let answers = answers.lock().unwrap();
        assert_eq!(answers[0], r#"{"ok":true}"#);
        assert!(answers[1].contains("error"));
        assert_eq!(answers.len(), 2);
    }
}
