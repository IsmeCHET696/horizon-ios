import SwiftUI
import UIKit

/// The C entry point `horizon-ios-launcher` looks up: show the launcher over the game, which keeps running
/// under it at its title screen. Play dismisses it. Called on the main thread once the game's window exists;
/// the presenting waits a turn of the run loop, because that window's view controller may not be on screen
/// yet when its first frame is being set up.
@_cdecl("horizon_launcher_present")
public func horizonLauncherPresent() -> Bool {
    Theme.registerFonts()
    DispatchQueue.main.async { present(attempts: 20) }
    return true
}

private func present(attempts: Int) {
    guard let root = gameViewController(), root.view.window != nil else {
        // Not on screen yet: try again shortly, for a couple of seconds at most.
        if attempts > 0 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { present(attempts: attempts - 1) }
        }
        return
    }
    let host = LauncherHostingController(rootView: AnyView(EmptyView()))
    host.rootView = AnyView(
        HomeView(onPlay: { [weak host] in host?.dismiss(animated: true) })
            .preferredColorScheme(.dark)
    )
    host.modalPresentationStyle = .fullScreen
    host.modalTransitionStyle = .crossDissolve
    root.present(host, animated: false)
}

/// The view controller of winit's window: the one the launcher is presented from.
private func gameViewController() -> UIViewController? {
    let windows = UIApplication.shared.connectedScenes
        .compactMap { $0 as? UIWindowScene }
        .flatMap(\.windows)
    let window = windows.first(where: \.isKeyWindow) ?? windows.first
    var top = window?.rootViewController
    while let presented = top?.presentedViewController {
        top = presented
    }
    return top
}

/// The game is held sideways (GameInfo.plist), and so is its launcher: a presented controller that allowed
/// portrait would turn the phone's picture upright over a landscape game.
private final class LauncherHostingController: UIHostingController<AnyView> {
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask { .landscape }
    override var preferredStatusBarStyle: UIStatusBarStyle { .lightContent }
}
