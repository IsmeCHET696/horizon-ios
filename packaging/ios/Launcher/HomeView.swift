import SwiftUI
import UIKit

/// The launcher's home screen, upright: the game, a Play button that uncovers it, and the player it plays as.
struct HomeView: View {
    /// Play: uncover the game (Present.swift).
    var onPlay: () -> Void
    @StateObject private var player = PlayerName()
    @StateObject private var head = SkinHead()
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
                GameCard(onPlay: onPlay)
                PlayerCard(name: player.name, face: head.face, edit: editName)
            }
            .padding(.horizontal, 24)
            .padding(.top, 12)
            .padding(.bottom, 24)
            .frame(maxWidth: 460)
            .frame(maxWidth: .infinity)
        }
        .safeAreaInset(edge: .top, spacing: 0) {
            Header(name: player.name, face: head.face, editName: editName)
                .frame(maxWidth: 460).frame(maxWidth: .infinity).background(Theme.background)
        }
        // The offline name the game plays with.
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
        // The face follows the name: the skin of the Minecraft account that has it.
        .task(id: player.name) { await head.load(player.name) }
    }

    private func editName() {
        nameDraft = player.name ?? ""
        editingName = true
    }
}

// MARK: - Header

private struct Header: View {
    let name: String?
    let face: UIImage?
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
            Button(action: editName) {
                Avatar(name: name, face: face, size: 40)
            }
            .accessibilityLabel("Player name: \(name ?? "not set")")
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

/// The player's skin face, its pixels kept sharp; the name's first letter until a skin is found.
private struct Avatar: View {
    let name: String?
    let face: UIImage?
    let size: CGFloat

    var body: some View {
        Group {
            if let face {
                Image(uiImage: face)
                    .resizable()
                    .interpolation(.none)
                    .padding(size * 0.12)
            } else {
                Text(name.map { String($0.prefix(1)).lowercased() } ?? "?")
                    .font(Theme.pixel(size * 0.52, .medium))
                    .foregroundStyle(Theme.text)
            }
        }
        .frame(width: size, height: size)
        .background(Theme.notice, in: RoundedRectangle(cornerRadius: size * 0.3))
        .overlay(RoundedRectangle(cornerRadius: size * 0.3).strokeBorder(Theme.border))
    }
}

// MARK: - The game

private struct GameCard: View {
    let onPlay: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            Hero()
            PlayButton(action: onPlay)
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
            HStack(spacing: 6) {
                Rectangle().fill(Theme.ready).frame(width: 5, height: 5)
                Text("MINECRAFT 26.3").font(Theme.mono(9)).foregroundStyle(Theme.text)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(Color(red: 16 / 255, green: 13 / 255, blue: 32 / 255, opacity: 0.8), in: RoundedRectangle(cornerRadius: 4))
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

// MARK: - The player

private struct PlayerCard: View {
    let name: String?
    let face: UIImage?
    let edit: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Avatar(name: name, face: face, size: 48)
            VStack(alignment: .leading, spacing: 4) {
                Text("PLAYING AS").font(Theme.mono(8)).foregroundStyle(Theme.textMuted)
                Text(name ?? "No name yet").font(Theme.sans(15, .semiBold)).foregroundStyle(Theme.text)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            Button(action: edit) {
                Text("Change").font(Theme.sans(12, .semiBold)).foregroundStyle(Theme.accentText)
                    .frame(minWidth: 44, minHeight: 44)
            }
        }
        .padding(14)
        .background(Theme.surface, in: RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).strokeBorder(Theme.border))
    }
}
