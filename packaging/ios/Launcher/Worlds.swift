import SwiftUI
import UIKit

extension World {
    /// "Survival · 2 hours ago · 26.3", as Select World's second line says it.
    var subtitle: String {
        let ago = RelativeDateTimeFormatter().localizedString(for: lastPlayed, relativeTo: Date())
        return [mode.capitalized, ago, version].filter { !$0.isEmpty }.joined(separator: " · ")
    }
}

/// A world's picture, or a grass-green square with its initial, pixels kept sharp.
struct WorldIcon: View {
    let world: World
    let size: CGFloat

    var body: some View {
        Group {
            if let icon = world.icon {
                Image(uiImage: icon).resizable().interpolation(.none).scaledToFill()
            } else {
                ZStack {
                    LinearGradient(colors: [Color(hex: 0x5D9A3A), Color(hex: 0x3F6E28)], startPoint: .top,
                                   endPoint: .bottom)
                    Text(String(world.name.prefix(1)).uppercased())
                        .font(Theme.pixel(size * 0.45, .bold)).foregroundStyle(.white)
                        .shadow(color: .black.opacity(0.4), radius: 0, x: 1, y: 1)
                }
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: 6))
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Theme.border))
    }
}

/// Every saved world: tap to play, hold for Rename, Back Up and Delete; New makes one.
struct WorldsSection: View {
    @ObservedObject var link: GameLink
    @State private var creating = false
    @State private var renaming: World?
    @State private var newName = ""
    @State private var deleting: World?
    @State private var backedUp: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            SectionHeader(title: "Worlds") {
                Button {
                    Haptics.tap()
                    creating = true
                } label: {
                    Label("New", systemImage: "plus").font(Theme.sans(13, .semiBold))
                        .foregroundStyle(Theme.accentText)
                        .frame(minHeight: 44)
                }
                .disabled(!link.ready)
            }
            if !link.ready, link.worlds.isEmpty {
                EmptyNote(icon: "hourglass", text: "Your worlds show here once the game has loaded.")
            } else if link.worlds.isEmpty {
                EmptyNote(icon: "map", text: "No worlds yet. Make one with New, or bring one in as a .zip.")
            } else {
                VStack(spacing: 0) {
                    ForEach(link.worlds) { world in
                        WorldRow(world: world) {
                            Task { await link.play(world) }
                        }
                        .contextMenu {
                            Button {
                                Task { await link.play(world) }
                            } label: { Label("Play", systemImage: "play.fill") }
                            Button {
                                newName = world.name
                                renaming = world
                            } label: { Label("Rename", systemImage: "pencil") }
                            Button {
                                Task {
                                    if let bytes = await link.backup(world) {
                                        let size = ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)
                                        backedUp = "\(world.name) was saved to the backups folder (\(size))."
                                        Haptics.done()
                                    }
                                }
                            } label: { Label("Back Up", systemImage: "externaldrive.badge.plus") }
                            Button(role: .destructive) {
                                deleting = world
                            } label: { Label("Delete", systemImage: "trash") }
                        }
                        if world != link.worlds.last {
                            Divider().overlay(Theme.border).padding(.leading, 70)
                        }
                    }
                }
                .card()
                Text("Hold a world to rename, back up or delete it.")
                    .font(Theme.sans(11)).foregroundStyle(Theme.textFaint)
            }
        }
        .sheet(isPresented: $creating) {
            NewWorldSheet { name, seed, creative in
                creating = false
                Task { await link.create(name: name, seed: seed, creative: creative) }
            }
        }
        .alert("Rename world", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("World name", text: $newName)
            Button("Rename") {
                if let world = renaming {
                    Task { await link.rename(world, to: newName) }
                }
            }
            Button("Cancel", role: .cancel) {}
        }
        .confirmationDialog(
            "Delete \(deleting?.name ?? "this world")?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }),
            titleVisibility: .visible
        ) {
            Button("Delete for good", role: .destructive) {
                if let world = deleting {
                    Task { await link.delete(world) }
                }
            }
        } message: {
            Text("It will be lost forever (a long time!). Back it up first if you might want it again.")
        }
        .alert("Backed up", isPresented: Binding(get: { backedUp != nil }, set: { if !$0 { backedUp = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(backedUp ?? "")
        }
    }
}

private struct WorldRow: View {
    let world: World
    let play: () -> Void

    var body: some View {
        Button(action: play) {
            HStack(spacing: 12) {
                WorldIcon(world: world, size: 46)
                VStack(alignment: .leading, spacing: 3) {
                    Text(world.name).font(Theme.sans(15, .semiBold)).foregroundStyle(Theme.text).lineLimit(1)
                    Text(world.subtitle).font(Theme.sans(11)).foregroundStyle(Theme.textMuted).lineLimit(1)
                }
                Spacer(minLength: 0)
                if world.mode == "creative" {
                    Image(systemName: "cube.fill").foregroundStyle(Theme.accentText).font(.system(size: 13))
                }
                Image(systemName: "chevron.right").foregroundStyle(Theme.textFaint).font(.system(size: 12, weight: .semibold))
            }
            .padding(12)
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
    }
}

/// Create New World, in short: a name, a game mode and an optional seed.
private struct NewWorldSheet: View {
    let create: (String, String, Bool) -> Void
    @State private var name = "New World"
    @State private var seed = ""
    @State private var creative = false
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                HStack {
                    Text("Create New World").font(Theme.pixel(24)).foregroundStyle(Theme.text)
                    Spacer()
                    Button("Cancel") { dismiss() }.font(Theme.sans(14)).foregroundStyle(Theme.textMuted)
                }
                Field(title: "WORLD NAME", placeholder: "New World", text: $name, keyboard: .default)
                VStack(alignment: .leading, spacing: 6) {
                    Text("GAME MODE").font(Theme.mono(9)).foregroundStyle(Theme.textMuted)
                    HStack(spacing: 8) {
                        ModeTile(title: "Survival", detail: "Gather, build, survive", icon: "heart.fill",
                                 selected: !creative) { creative = false }
                        ModeTile(title: "Creative", detail: "Every block, no limits", icon: "cube.fill",
                                 selected: creative) { creative = true }
                    }
                }
                Field(title: "SEED (OPTIONAL)", placeholder: "Leave blank for a random seed", text: $seed,
                      keyboard: .asciiCapable)
                Text("A seed is a number or a word without spaces; the same seed makes the same world.")
                    .font(Theme.sans(11)).foregroundStyle(Theme.textFaint)
                PrimaryButton(title: "Create World", icon: "sparkles") {
                    create(name, seed, creative)
                }
            }
            .padding(20)
        }
        .background(Theme.background.ignoresSafeArea())
        .presentationDetents([.medium, .large])
        .preferredColorScheme(.dark)
    }
}

private struct ModeTile: View {
    let title: String
    let detail: String
    let icon: String
    let selected: Bool
    let pick: () -> Void

    var body: some View {
        Button {
            Haptics.tap()
            pick()
        } label: {
            VStack(alignment: .leading, spacing: 6) {
                Image(systemName: icon).foregroundStyle(selected ? Theme.accentText : Theme.textFaint)
                Text(title).font(Theme.pixel(17)).foregroundStyle(Theme.text)
                Text(detail).font(Theme.sans(11)).foregroundStyle(Theme.textMuted)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(selected ? Theme.notice : Theme.surfaceSunken, in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).strokeBorder(selected ? Theme.accent : Theme.border,
                                                                     lineWidth: selected ? 2 : 1))
        }
        .buttonStyle(PressStyle())
    }
}
