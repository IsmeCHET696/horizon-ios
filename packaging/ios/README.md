# The iOS app shell: what Horizon's simulation says on an iPhone

Three files and one crate make the app shell, and a fourth puts the game itself in a bundle beside it.

- `project.yml`: the Xcode project, as an [XcodeGen](https://github.com/yonaskolb/XcodeGen) spec. Run
  `brew install xcodegen && xcodegen generate` in this folder to write `HorizonIOS.xcodeproj`, which is
  not checked in. Its paths are relative to this directory, so generate it from here.
- `Info.plist`: the bundle's plist, filled in by Xcode from `project.yml`'s `MARKETING_VERSION` and
  `CURRENT_PROJECT_VERSION`. Bundle id `dev.horizon.ios`, iOS 16 or later, iPhone and iPad.
- `Sources/main.m`: the app. One `AppDelegate`, one view controller, and no storyboard: it calls
  `horizon_probe()` in `viewDidLoad` and shows what came back in a monospaced text view.
- `GameInfo.plist`: a second bundle's plist, for the `.ipa` whose executable is the cross-compiled
  `horizon-client` rather than an app written here. Bundle id `dev.horizon.game`, and literal versions because no
  Xcode substitutes anything. See "The second bundle" below.

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

## The second bundle: the game, with no Xcode in its build

`.github/workflows/ios.yml` uploads two `.ipa` files, because two things are being measured. `Horizon-unsigned.ipa`
is the shell above. `Horizon-game-unsigned.ipa` is `horizon-client` itself:

```sh
cargo build --release --locked --target aarch64-apple-ios -p horizon-client --no-default-features
rm -rf game-payload && mkdir -p game-payload/Payload/Horizon.app
cp target/aarch64-apple-ios/release/horizon-client game-payload/Payload/Horizon.app/Horizon
cp GameInfo.plist game-payload/Payload/Horizon.app/Info.plist
printf 'APPL????' > game-payload/Payload/Horizon.app/PkgInfo
( cd game-payload && zip -qry ../Horizon-game-unsigned.ipa Payload )
```

No `.xcodeproj` and no `xcodegen`: rustc produced an executable, and the rest is a folder layout iOS loads. The
bundle id is `dev.horizon.game`, so the two install side by side. `GameInfo.plist` carries literal versions where
`Info.plist` carries `$(MARKETING_VERSION)` for Xcode to fill in -- keep the two in step with `project.yml`.

What that bundle cannot do yet, in the order it will be found: it has **no assets** (CI has no Minecraft jar to run
`horizon-prepare` against, and this repository must not ship one; `UIFileSharingEnabled` in the plist is the
deliberate door for the player to put the prepared folder where the game looks), and **no touch input** -- the first
frame may be the whole show until something turns a tap into a key. The game starts in `$HOME/Documents`, which is
why `saves/`, `options.txt` and `.horizon-cache` are where a player can reach them; see `crates/horizon-client/src/main.rs`
`into_app_sandbox`.

## Running it on a phone

An unsigned `.ipa` installs nowhere, so the route that works today is the one Xcode signs for you. `project.yml`
has two application targets and two schemes:

| Scheme | Bundle id | Executable | What it is |
|---|---|---|---|
| **Horizon shell** | `dev.horizon.ios` | Objective-C + `libhorizon_ios.a` | the four probes, printed on screen |
| **Horizon game** | `dev.horizon.game` | Cargo's `horizon-client` | the game |

```sh
rustup target add aarch64-apple-ios
brew install xcodegen
cd packaging/ios && xcodegen generate && open HorizonIOS.xcodeproj
```

Pick **Horizon game**, choose your device, set a signing team on the target, and press Run. The target's first
build phase runs `cargo build --release --target aarch64-apple-ios -p horizon-client --no-default-features`;
Xcode then links a placeholder `GameShell/main.m` so the bundle has an executable, and a phase after it
overwrites that file with Cargo's binary -- which is what the signature then covers. rustc cannot emit a library
from a bin target and `Game` is bin-only code, so this is the only shape that gets the game into a signed bundle
without moving 30k lines into the library. `.github/workflows/ios.yml` builds the same target with
`CODE_SIGNING_ALLOWED=NO` and `cmp`s the bundle's executable against Cargo's, so "the copy stuck" is a checked
claim and not an assumption.

**If you open the shell and see the probe report, that is the other target.** The report text is
`crates/horizon-ios`, printed by `Sources/main.m`; nothing about it is the game.

### Landscape, which two places have to say

The game is landscape-only on a phone, and both halves are needed:

- `UISupportedInterfaceOrientations` in `Info.plist` and `GameInfo.plist` lists the two landscape directions and
  no portrait. This is what the launch screen and the bundle's own metadata are judged by.
- `ValidOrientations::Landscape` on the winit window attributes (`crates/horizon-client/src/game.rs`, both
  `resumed` implementations). Without it the plist alone does not hold: winit implements
  `supportedInterfaceOrientations` on its own view controller from its default
  (`LandscapeAndPortrait`, and it never reads the plist), and a phone asks the root view controller rather than
  the bundle, so the game would rotate into portrait under it.

Both landscape directions, not one: a handheld gets to be held either way, and the window size is the same in
each, so nothing in the render path has to care.

### Where the game looks, on the phone

- It works in `$HOME/Documents`, which on iOS is the app's data container (`main.rs` `into_app_sandbox`). Saves
  land in `Documents/saves`, settings in `Documents/options.txt`, pack caches in `Documents/.horizon-cache`.
- The log is `Library/Application Support/horizon/logs/latest.log` -- the container Xcode's *Devices and
  Simulators* window downloads, and the way to read what happened when nothing appeared.
- Assets: `Options::read` looks for `horizon-resources.json` in a `resources` folder **beside the executable**,
  which on a phone means inside the bundle. In Xcode, add your prepared `.local/horizon-client/26.3` folder to
  the **Horizon game** target as a folder reference with a path in bundle of `resources` (produce folders: keep
  their name or rename it there). Without it the game has no blocks, no textures and no language file, and says
  so in the log rather than crashing quietly.
- The fallback path is `.local/horizon-client/26.3` under the working directory, i.e.
  `Documents/.local/horizon-client/26.3`. The Files app hides dotfolders, which is why the bundle is the route
  written above. `UIFileSharingEnabled` in `GameInfo.plist` is what makes `Documents` visible there at all.

Nobody has run this on a device yet. What is measured is in the workflow's summary: the game compiles for the
target, links, and comes out of Xcode's bundle as the same bytes Cargo wrote.

## Why the game, when it comes, is the Cargo executable and not another static library

`crates/horizon-ios` is a library, so `cargo rustc --crate-type staticlib` works for it. The game is not a
library: `Game` and `game::run()` live in the **bin** target (`crates/horizon-client/src/main.rs`, `game.rs`,
`src/game/`), and the lib cannot see them. rustc will not emit a library from a bin -- both
`crate-type = ["staticlib"]` in a `[[bin]]` and `cargo rustc --bin … --crate-type staticlib` are hard errors
("crate types can only be specified for libraries and example libraries"). winit 0.30's iOS backend removes the
need for that detour anyway: `EventLoop::run_app` calls `UIApplicationMain` itself, so a Rust executable built
by `cargo build --target aarch64-apple-ios` is a legitimate `Payload/Horizon.app/Horizon`, which is what
`.github/workflows/ios.yml` now produces beside this shell.
