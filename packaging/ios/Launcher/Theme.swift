import CoreText
import SwiftUI

// The phone's launcher: the home screen from the Figma design (UI, node 40:168), drawn with Horizon's own
// fonts from crates/horizon-theme/fonts. It is a framework the game's bundle embeds, not an app: the game owns
// the process, and shows this over itself through Present.swift. Play uncovers the game; the other buttons are
// still placeholders.

/// The framework's own bundle: its images and fonts are there, not in the app's.
final class Kit {
    static let bundle = Bundle(for: Kit.self)

    static func image(_ name: String) -> Image {
        Image(name, bundle: bundle)
    }
}

/// The design's colours and type, named once so the views read as the design does.
enum Theme {
    static let background = Color(hex: 0x0C0B12)
    static let surface = Color(hex: 0x191624)
    static let surfaceSunken = Color(hex: 0x100D1A)
    static let surfaceDisabled = Color(hex: 0x221D32)
    static let border = Color(hex: 0x302A42)
    static let accent = Color(hex: 0x7645F5)
    static let accentEdge = Color(hex: 0xA68BFF)
    static let accentBright = Color(hex: 0x8960FF)
    static let accentText = Color(hex: 0xC2AAFF)
    static let notice = Color(hex: 0x2E2149)
    static let noticeBorder = Color(hex: 0x483460)
    static let ready = Color(hex: 0x5BD7A0)
    static let text = Color(hex: 0xF5F1FF)
    static let textMuted = Color(hex: 0xA9A1BC)
    static let textFaint = Color(hex: 0x777085)

    // The design names Inter and Roboto Mono; Horizon's launcher speaks Geist and Geist Mono, which ship in
    // the repository, so the phone matches the desktop rather than a font the app would have to bundle anew.
    static func pixel(_ size: CGFloat, _ weight: PixelWeight = .semiBold) -> Font {
        .custom(weight.rawValue, fixedSize: size)
    }
    static func sans(_ size: CGFloat, _ weight: SansWeight = .regular) -> Font {
        .custom(weight.rawValue, fixedSize: size)
    }
    static func mono(_ size: CGFloat) -> Font {
        .custom("GeistMono-Regular", fixedSize: size)
    }

    enum PixelWeight: String {
        case medium = "PixelifySans-Medium"
        case semiBold = "PixelifySans-SemiBold"
        case bold = "PixelifySans-Bold"
    }
    enum SansWeight: String {
        case regular = "Geist-Regular"
        case medium = "Geist-Medium"
        case semiBold = "Geist-SemiBold"
    }

    /// The fonts are bundle resources and not `UIAppFonts` entries, so the generated Info.plist needs no list
    /// that could drift from the folder.
    static func registerFonts() {
        for url in Kit.bundle.urls(forResourcesWithExtension: "ttf", subdirectory: nil) ?? [] {
            CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
        }
    }
}

extension Color {
    init(hex: UInt32, opacity: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: opacity
        )
    }
}
