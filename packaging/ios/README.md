# The iOS app shell: what Horizon's simulation says on an iPhone

Three files and one crate, and none of them is the game.

- `project.yml`: the Xcode project, as an [XcodeGen](https://github.com/yonaskolb/XcodeGen) spec. Run
  `brew install xcodegen && xcodegen generate` in this folder to write `HorizonIOS.xcodeproj`, which is
  not checked in. Its paths are relative to this directory, so generate it from here.
- `Info.plist`: the bundle's plist, filled in by Xcode from `project.yml`'s `MARKETING_VERSION` and
  `CURRENT_PROJECT_VERSION`. Bundle id `dev.horizon.ios`, iOS 16 or later, iPhone and iPad.
- `Sources/main.m`: the app. One `AppDelegate`, one view controller, and no storyboard: it calls
  `horizon_probe()` in `viewDidLoad` and shows what came back in a monospaced text view.

The Rust half is [`crates/horizon-ios`](../../crates/horizon-ios), whose header is
`crates/horizon-ios/include/horizon_ios.h`. The project links it by name out of
`target/aarch64-apple-ios/release`, so nothing here pretends a fresh clone is buildable: the archive
has to be cross-compiled first.

## Building it, as the workflow does

```sh
cargo rustc --release --locked --target aarch64-apple-ios -p horizon-ios --lib --crate-type staticlib
cd packaging/ios && xcodegen generate && cd ../..
xcodebuild -project packaging/ios/HorizonIOS.xcodeproj -target Horizon -configuration Release \
  -sdk iphoneos -arch arm64 \
  CODE_SIGNING_ALLOWED=NO CODE_SIGN_IDENTITY= CODE_SIGNING_REQUIRED=NO \
  CONFIGURATION_BUILD_DIR="$PWD/build-ios" build
mkdir -p ipa/Payload && cp -a build-ios/Horizon.app ipa/Payload/
( cd ipa && zip -qry ../Horizon-unsigned.ipa Payload )
```

`.github/workflows/ios.yml` does exactly this on a Mac runner, checks the bundle's binary is arm64
iOS machine code with `_horizon_probe` really linked into it, and uploads `Horizon-unsigned.ipa`.

## What the .ipa is, and what it is not

It is **unsigned**: iOS refuses to install it, on your own phone included. What it is for is proof of
the chain — Cargo's arm64 objects, an Objective-C caller, a linker, a bundle, a Payload archive — so
that the only thing standing between it and a hand is a signing certificate, which is an account and
not a build problem. With any Apple ID added to Xcode, open the generated project, pick a team, and
run it on the device.

It is **not the game**: this bundle holds the probe crate, not `horizon-client`. The client now *compiles*
for `aarch64-apple-ios` -- the Mac runner measured that on 2026-10-24, with `--no-default-features`, and the
clipboard that used to drag the Wayland stack into a phone build is gated to the four targets `arboard` has
a backend for (`crates/horizon-client/Cargo.toml`). Compiling is the entry fee and not the port. What stands
between this bundle and a playable one:

| Gap | Why it is structural | Decided |
|---|---|---|
| Assets | `horizon-prepare` runs a JDK against the player's own Minecraft install to produce `.local/horizon-client/26.3`; there is no Java on iOS | Prepare off-device, download to the phone |
| Mods | `horizon-mods` runs them through wasmtime's cranelift JIT, and Apple forbids generating executable code at runtime (`docs/mods.md`) -- note the runner says the `mods` *feature compiles*, which is about the compiler and not about running | Disabled on iOS |
| Input | the client's winit loop is desktop-shaped: cursor-warp mouse look, no touch | A touch layer, in no crate yet |
| Sound, controllers | `cpal`'s CoreAudio backend is macOS's and `hidapi`/`gilrs` need libudev or IOKit, which is what `--no-default-features` drops and why the phone build asks for that flag | `AVAudioEngine` and `GCController`, neither written yet |

So this app runs the half that was already portable before the client was: `horizon-core`'s Java-accurate RNG
and HotSpot-exact `Math`, its paletted block storage, and `horizon-world`'s NBT codec -- the four probes whose
answers you can read on the screen, all of them computed on the phone rather than shipped to it.

## Why the game, when it comes, is the Cargo executable and not another static library

`crates/horizon-ios` is a library, so `cargo rustc --crate-type staticlib` works for it. The game is not a
library: `Game` and `game::run()` live in the **bin** target (`crates/horizon-client/src/main.rs`, `game.rs`,
`src/game/`), and the lib cannot see them. rustc will not emit a library from a bin -- both
`crate-type = ["staticlib"]` in a `[[bin]]` and `cargo rustc --bin … --crate-type staticlib` are hard errors
("crate types can only be specified for libraries and example libraries"). winit 0.30's iOS backend removes the
need for that detour anyway: `EventLoop::run_app` calls `UIApplicationMain` itself, so a Rust executable built
by `cargo build --target aarch64-apple-ios` is a legitimate `Payload/Horizon.app/Horizon`, which is what
`.github/workflows/ios.yml` now produces beside this shell.
