import SwiftUI

/// The launcher's home screen. Placeholder state until the launcher has a backend on the phone.
struct HomeView: View {
    /// Play: uncover the game (Present.swift).
    var onPlay: () -> Void
    @State private var inviteCode = ""
    @StateObject private var player = PlayerName()
    @State private var editingName = false
    @State private var nameDraft = ""
    @State private var nameRejected = false

    // Spelled out: a private property makes the memberwise one private, and Present.swift builds this view.
    init(onPlay: @escaping () -> Void = {}) {
        self.onPlay = onPlay
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                InstanceSelector()
                GameCard(onPlay: onPlay)
                UpdateNotice(version: "v0.2.50", note: "Fix periods swallowed by debug shortcuts in server address fields (#1577)", progress: 0.69)
                    .padding(.top, 8)
                Preparation()
                    .padding(.top, 8)
                JoinFriend(code: $inviteCode)
                    .padding(.top, 8)
                Footer(name: player.name)
                    .padding(.top, 8)
            }
            .padding(.horizontal, 24)
            .padding(.top, 12)
            .padding(.bottom, 24)
            // The design is a phone held upright; the game holds it sideways, so the column keeps its width
            // and sits in the middle.
            .frame(maxWidth: 460)
            .frame(maxWidth: .infinity)
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            Header(name: player.name) {
                nameDraft = player.name ?? ""
                editingName = true
            }
            .frame(maxWidth: 460).frame(maxWidth: .infinity).background(Theme.background)
        }
        // The account button: the offline name the game plays with.
        .alert("Player name", isPresented: $editingName) {
            TextField("Name", text: $nameDraft)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Button("Save") {
                nameRejected = !player.save(nameDraft.trimmingCharacters(in: .whitespaces))
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("Your name in single player and on servers that don't check accounts. Up to 16 characters, no spaces.")
        }
        .alert("That name can't be used", isPresented: $nameRejected) {
            Button("OK", role: .cancel) {}
        } message: {
            Text("Use 1 to 16 letters, digits or symbols, with no spaces.")
        }
        .background(Theme.background.ignoresSafeArea())
    }
}

// MARK: - Header

private struct Header: View {
    let name: String?
    let editName: () -> Void

    var body: some View {
        HStack {
            HStack(spacing: 10) {
                HorizonMark()
                Text("HORIZON")
                    .font(Theme.pixel(25, .bold))
                    .foregroundStyle(Theme.text)
            }
            Spacer()
            HStack(spacing: 8) {
                Button(action: editName) {
                    Text(name.map { String($0.prefix(1)).lowercased() } ?? "?")
                        .font(Theme.pixel(21, .medium))
                        .foregroundStyle(Theme.text)
                        .frame(width: 40, height: 40)
                        .background(Theme.notice, in: RoundedRectangle(cornerRadius: 12))
                        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.border))
                        .overlay(alignment: .topLeading) {
                            Circle()
                                .fill(Theme.textFaint)
                                .frame(width: 9, height: 9)
                                .overlay(Circle().strokeBorder(Theme.background, lineWidth: 2))
                                .offset(x: 28, y: 28)
                        }
                }
                .accessibilityLabel("Player name: \(name ?? "not set"), offline")
                IconButton(image: "sliders-horizontal", label: "Settings") {}
            }
        }
        .padding(.horizontal, 24)
        .frame(height: 66)
        .background(Theme.background)
    }
}

/// Horizon's plus-shaped mark: a violet horizon crossed by a white stroke.
private struct HorizonMark: View {
    var body: some View {
        ZStack {
            Rectangle().fill(Theme.accentBright).frame(width: 22, height: 3)
            Rectangle().fill(Theme.text).frame(width: 3, height: 22)
        }
        .frame(width: 22, height: 22)
    }
}

private struct IconButton: View {
    let image: String
    let label: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Kit.image(image)
                .frame(width: 44, height: 44)
                .background(Theme.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.border))
        }
        .accessibilityLabel(label)
    }
}

// MARK: - Instance and game

private struct InstanceSelector: View {
    var body: some View {
        HStack(spacing: 12) {
            Button {} label: {
                HStack(spacing: 12) {
                    GrassBlock()
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Horizon").font(Theme.sans(14)).foregroundStyle(Theme.text)
                        Text("Vanilla · ready").font(Theme.sans(11)).foregroundStyle(Theme.textMuted)
                    }
                    .lineLimit(1)
                    Spacer(minLength: 0)
                    Kit.image("chevrons-up-down")
                }
                .padding(12)
                .background(Theme.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.border))
            }
            IconButton(image: "plus", label: "Add instance") {}
        }
    }
}

/// The instance's icon: a grass block drawn in pixels, as the design draws it.
private struct GrassBlock: View {
    private static let pixels: [(x: CGFloat, y: CGFloat, w: CGFloat, h: CGFloat, color: UInt32)] = [
        (0, 0, 38, 13, 0x79AD46),
        (0, 10, 8, 10, 0x5E903C),
        (17, 10, 10, 7, 0x5E903C),
        (5, 25, 7, 5, 0xAD7A4D),
        (25, 20, 8, 5, 0x4D382A),
        (16, 31, 6, 5, 0xAA8055),
        (28, 31, 7, 5, 0x62432F),
        (4, 3, 9, 5, 0x95C65D),
        (22, 5, 9, 5, 0x568534),
    ]

    var body: some View {
        ZStack(alignment: .topLeading) {
            Color(hex: 0x795137)
            ForEach(Self.pixels.indices, id: \.self) { i in
                let p = Self.pixels[i]
                Rectangle()
                    .fill(Color(hex: p.color))
                    .frame(width: p.w, height: p.h)
                    .offset(x: p.x, y: p.y)
            }
        }
        .frame(width: 38, height: 38)
        .clipShape(RoundedRectangle(cornerRadius: 4))
    }
}

private struct GameCard: View {
    let onPlay: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            Hero()
            VStack(spacing: 14) {
                HStack(spacing: 10) {
                    VersionPicker()
                    PlayButton(action: onPlay)
                }
                Button {} label: {
                    HStack(spacing: 8) {
                        Kit.image("radio-tower")
                        Text("Host a server").font(Theme.sans(12, .medium)).foregroundStyle(Theme.textMuted)
                        Kit.image("arrow-up-right")
                    }
                    .frame(maxWidth: .infinity, minHeight: 24)
                }
            }
            .padding(16)
        }
        .background(Theme.surface)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Theme.border))
    }
}

private struct Hero: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                HStack(spacing: 6) {
                    Rectangle().fill(Theme.ready).frame(width: 5, height: 5)
                    Text("MINECRAFT 26.3").font(Theme.mono(9)).foregroundStyle(Theme.text)
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 6)
                .background(Color(red: 16 / 255, green: 13 / 255, blue: 32 / 255, opacity: 0.8), in: RoundedRectangle(cornerRadius: 4))
                Spacer()
                Text("60 PLAYING NOW").font(Theme.mono(9)).foregroundStyle(Color(hex: 0xE0D5F5))
            }
            VStack(alignment: .leading, spacing: 6) {
                Text("Horizon").font(Theme.pixel(44)).foregroundStyle(Theme.text)
                Text("Your next world is waiting.").font(Theme.sans(12)).foregroundStyle(Color(hex: 0xDDD2ED))
            }
            .padding(.top, 35)
            Spacer(minLength: 0)
        }
        .padding(20)
        .frame(maxWidth: .infinity, minHeight: 214, maxHeight: 214, alignment: .topLeading)
        .background {
            Kit.image("pixel-sunset")
                .resizable()
                .interpolation(.none)
                .scaledToFill()
                .overlay(
                    LinearGradient(
                        colors: [
                            Color(red: 16 / 255, green: 12 / 255, blue: 34 / 255, opacity: 0.2),
                            Color(red: 16 / 255, green: 12 / 255, blue: 34 / 255, opacity: 0.91),
                        ],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                )
        }
        .clipped()
    }
}

private struct VersionPicker: View {
    var body: some View {
        Menu {
            Button("v0.2.50") {}
            Button("v0.2.49") {}
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("VERSION").font(Theme.mono(8)).foregroundStyle(Theme.textMuted)
                    Text("v0.2.49").font(Theme.mono(12)).foregroundStyle(Theme.text)
                }
                Spacer(minLength: 0)
                Kit.image("chevron-down")
            }
            .padding(.horizontal, 12)
            .frame(width: 116, height: 54)
            .background(Theme.surfaceSunken, in: RoundedRectangle(cornerRadius: 8))
            .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Theme.border))
        }
    }
}

private struct PlayButton: View {
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Kit.image("play")
                Text("Play").font(Theme.pixel(27)).foregroundStyle(Theme.text)
            }
            .frame(maxWidth: .infinity, minHeight: 54, maxHeight: 54)
            .background(Theme.accent)
            // The design's lit top edge: a one-point border on the top side only.
            .overlay(alignment: .top) { Theme.accentEdge.frame(height: 1) }
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }
}

// MARK: - Utilities

private struct UpdateNotice: View {
    let version: String
    let note: String
    let progress: Double

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                HStack(spacing: 7) {
                    Rectangle().fill(Theme.accentBright).frame(width: 5, height: 5)
                    Text("\(version) available").font(Theme.sans(11, .semiBold)).foregroundStyle(Theme.text)
                }
                Spacer()
                Button {} label: {
                    HStack(spacing: 4) {
                        Text("Patch notes").font(Theme.sans(10, .semiBold)).foregroundStyle(Theme.accentText)
                        Kit.image("chevron-right")
                    }
                }
            }
            Text(note)
                .font(Theme.sans(10))
                .lineSpacing(5)
                .foregroundStyle(Theme.textMuted)
                .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 10) {
                ProgressTrack(progress: progress)
                Text("\(Int((progress * 100).rounded()))%").font(Theme.mono(9)).foregroundStyle(Theme.accentText)
            }
        }
        .padding(14)
        .background(Theme.notice, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.noticeBorder))
    }
}

private struct ProgressTrack: View {
    let progress: Double

    var body: some View {
        GeometryReader { proxy in
            ZStack(alignment: .leading) {
                RoundedRectangle(cornerRadius: 2).fill(Theme.surfaceSunken)
                RoundedRectangle(cornerRadius: 2).fill(Theme.accentBright)
                    .frame(width: proxy.size.width * progress)
            }
        }
        .frame(height: 4)
    }
}

private struct Preparation: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Kit.image("check")
                Text("Ready to play").font(Theme.pixel(20, .medium)).foregroundStyle(Theme.text)
            }
            VStack(spacing: 0) {
                UtilityRow(icon: "package-check", title: "Resources", detail: Text("Prepared for Minecraft 26.3").font(Theme.sans(10)), action: "Prepare again")
                Theme.border.frame(height: 1)
                UtilityRow(icon: "folder", title: "Minecraft Folder", detail: Text("~/.local/share/PrismLauncher").font(Theme.mono(8)), action: "Choose…")
            }
            .background(Theme.surface, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.border))
        }
    }
}

private struct UtilityRow: View {
    let icon: String
    let title: String
    let detail: Text
    let action: String

    var body: some View {
        HStack(spacing: 10) {
            Kit.image(icon)
            VStack(alignment: .leading, spacing: 5) {
                Text(title).font(Theme.sans(13, .semiBold)).foregroundStyle(Theme.text)
                detail.foregroundStyle(Theme.textMuted)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Button {} label: {
                Text(action).font(Theme.sans(10, .semiBold)).foregroundStyle(Theme.accentBright)
                    .frame(minWidth: 44, minHeight: 44, alignment: .trailing)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 14)
    }
}

private struct JoinFriend: View {
    @Binding var code: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Join a friend").font(Theme.pixel(20)).foregroundStyle(Theme.text)
            Text("Paste the code a friend shows when they invite you.").font(Theme.sans(11)).foregroundStyle(Theme.textMuted)
            HStack(spacing: 8) {
                HStack(spacing: 9) {
                    Kit.image("link")
                    TextField("", text: $code, prompt: Text("Paste an invite code").foregroundColor(Theme.textFaint))
                        .font(Theme.sans(12))
                        .foregroundStyle(Theme.text)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }
                .padding(.horizontal, 14)
                .frame(height: 48)
                .background(Theme.surface, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.border))
                let enabled = !code.isEmpty
                Button {} label: {
                    Text("Join").font(Theme.sans(13, .semiBold))
                        .foregroundStyle(enabled ? Theme.text : Theme.textFaint)
                        .frame(width: 68, height: 48)
                        .background(enabled ? Theme.accent : Theme.surfaceDisabled, in: RoundedRectangle(cornerRadius: 12))
                }
                .disabled(!enabled)
            }
        }
    }
}

private struct Footer: View {
    let name: String?

    var body: some View {
        HStack {
            HStack(spacing: 16) {
                FooterLink(icon: "message-square", title: "Discord")
                FooterLink(icon: "globe", title: "Website")
            }
            Spacer()
            VStack(alignment: .trailing, spacing: 5) {
                Text("\(name ?? "No name yet") · Offline").font(Theme.sans(10)).foregroundStyle(Theme.textMuted)
                Text("LAUNCHER 0.2.5").font(Theme.mono(8)).foregroundStyle(Theme.textFaint)
            }
        }
    }
}

private struct FooterLink: View {
    let icon: String
    let title: String

    var body: some View {
        Button {} label: {
            HStack(spacing: 5) {
                Kit.image(icon)
                Text(title).font(Theme.sans(11)).foregroundStyle(Theme.textMuted)
            }
            .frame(height: 44)
        }
    }
}
