//! The phone's launcher, in the game's own bundle. The game owns the app (winit runs UIKit's main loop and
//! never hands it back), so the launcher cannot run before it: Xcode embeds the SwiftUI launcher as
//! `Frameworks/HorizonLauncherKit.framework`, and once the game's window is up [`present`] loads it and asks
//! it to cover the game. Its Play button uncovers the game, at its title screen.
//!
//! Loaded at run time rather than linked, so a bundle without the framework (the workflow's hand-assembled
//! one, a desktop build) is the game as before. The game forbids `unsafe`, so the loading lives here.

/// Load the launcher and show it over the game, once per run: the loading screen's window asks, and so does
/// the game's when it makes its own. False when this build or bundle has none, or it was already shown.
pub fn present() -> bool {
    static SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if SHOWN.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    #[cfg(target_os = "ios")]
    return ios::present();
    #[cfg(not(target_os = "ios"))]
    false
}

#[cfg(target_os = "ios")]
mod ios {
    use std::ffi::CString;

    /// `horizon_launcher_present` in packaging/ios/Launcher/Present.swift.
    const SYMBOL: &std::ffi::CStr = c"horizon_launcher_present";

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
        // `@_cdecl` Swift function taking nothing and returning a Bool, called on the main thread (the game's
        // window event), which is where UIKit has to be touched.
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
            let present: extern "C" fn() -> bool = std::mem::transmute(symbol);
            present()
        }
    }
}
