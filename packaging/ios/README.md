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

It is **not the game**. `horizon-client` does not compile for `aarch64-apple-ios`: its clipboard
dependency `arboard` has no iOS backend and falls back to the Linux one, which pulls the Wayland stack
into a phone build (`cargo tree --target aarch64-apple-ios -i wayland-backend`). That is the first
thing to fix and the least of the work. Behind it are three structural gaps, each with a decision
already taken:

| Gap | Why it is structural | Decided |
|---|---|---|
| Assets | `horizon-prepare` runs a JDK against the player's own Minecraft install to produce `.local/horizon-client/26.3`; there is no Java on iOS | Prepare off-device, download to the phone |
| Mods | `horizon-mods` runs them through wasmtime's cranelift JIT, and Apple forbids generating executable code at runtime (`docs/mods.md`) | Disabled on iOS |
| Input | the client's winit loop is desktop-shaped: cursor-warp mouse look, no touch | A touch layer, in no crate yet |

So this app runs the half that is already portable: `horizon-core`'s Java-accurate RNG and
HotSpot-exact `Math`, its paletted block storage, and `horizon-world`'s NBT codec — the four probes
whose answers you can read on the screen, all of them computed on the phone rather than shipped to it.
