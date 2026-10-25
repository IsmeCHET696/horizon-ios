import SwiftUI
import UIKit

/// The launcher, upright: Play (the game, its worlds and the player), Servers, Packs, Gallery and Settings
/// (What's New is in Settings). Everything it knows about the game it asks the game under it (GameLink.swift,
/// Bridge.swift).
struct HomeView: View {
    /// Play: uncover the game (Present.swift).
    var onPlay: () -> Void
    @StateObject private var link = GameLink()
    @StateObject private var device = DeviceStatus()
    @StateObject private var download = GameFilesDownload()
    @StateObject private var player = PlayerName()
    @StateObject private var head = SkinHead()
    @State private var editingName = false
    @State private var nameDraft = ""
    @State private var nameRejected = false
    @State private var tab = Tab.play

    enum Tab: Hashable {
        case play, servers, packs, gallery, settings
    }

    // Spelled out: a private property makes the memberwise one private, and Present.swift builds this view.
    init(onPlay: @escaping () -> Void = {}) {
        self.onPlay = onPlay
        // The bar under the pages, in the launcher's colours rather than the system's grey.
        let bar = UITabBarAppearance()
        bar.configureWithOpaqueBackground()
        bar.backgroundColor = UIColor(Theme.background)
        bar.shadowColor = UIColor(Theme.border)
        UITabBar.appearance().standardAppearance = bar
        UITabBar.appearance().scrollEdgeAppearance = bar
    }

    var body: some View {
        TabView(selection: $tab) {
            page(refresh: { await link.reload() }) { PlayPage(link: link, device: device, download: download, player: player, head: head,
                            editName: editName) }
                .tabItem { Label("Play", systemImage: "play.fill") }
                .tag(Tab.play)
            page(refresh: { await link.reloadServers() }) { ServersPage(link: link) }
                .tabItem { Label("Servers", systemImage: "globe") }
                .tag(Tab.servers)
            page { PacksPage(link: link) }
                .tabItem { Label("Packs", systemImage: "square.stack.3d.up.fill") }
                .tag(Tab.packs)
            page { GalleryPage() }
                .tabItem { Label("Gallery", systemImage: "photo.on.rectangle") }
                .tag(Tab.gallery)
            page { SettingsPage(link: link, download: download) }
                .tabItem { Label("Settings", systemImage: "slider.horizontal.3") }
                .tag(Tab.settings)
        }
        .tint(Theme.accentText)
        .onChange(of: tab) { _ in Haptics.tap() }
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
        .alert("Something went wrong", isPresented: Binding(
            get: { link.problem != nil },
            set: { if !$0 { link.problem = nil } }
        )) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(link.problem ?? "")
        }
        // The face follows the name: the skin of the Minecraft account that has it.
        .task(id: player.name) { await head.load(player.name) }
        .task {
            link.uncover = onPlay
            await link.watch()
        }
    }

    /// One tab's page: the header bar over a scrolling column, on the launcher's background.
    private func page<Content: View>(refresh: (() async -> Void)? = nil,
                                     @ViewBuilder _ content: () -> Content) -> some View {
        ScrollView {
            content().pageColumn()
        }
        .refreshable { await refresh?() }
        .safeAreaInset(edge: .top, spacing: 0) {
            Header(name: player.name, face: head.face, editName: editName)
                .frame(maxWidth: 520).frame(maxWidth: .infinity).background(Theme.background)
        }
        .background(Theme.background.ignoresSafeArea())
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
        .padding(.horizontal, 20)
        .frame(height: 62)
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
struct Avatar: View {
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

// MARK: - Play

private struct PlayPage: View {
    @ObservedObject var link: GameLink
    @ObservedObject var device: DeviceStatus
    @ObservedObject var download: GameFilesDownload
    @ObservedObject var player: PlayerName
    @ObservedObject var head: SkinHead
    let editName: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            GameCard(link: link, device: device)
            if case .needsFiles = link.phase {
                GameFilesCard(download: download, missing: true)
            }
            if let last = link.worlds.first {
                ContinueCard(world: last, link: link)
            }
            WorldsSection(link: link)
            PlayerCard(name: player.name, face: head.face, edit: editName)
        }
        .animation(.easeInOut(duration: 0.25), value: link.phase)
        .animation(.easeInOut(duration: 0.25), value: link.worlds)
    }
}

private struct GameCard: View {
    @ObservedObject var link: GameLink
    @ObservedObject var device: DeviceStatus

    var body: some View {
        VStack(spacing: 0) {
            Hero(version: link.version)
            VStack(spacing: 12) {
                StatusChips(link: link, device: device)
                PlayButton(link: link)
            }
            .padding(16)
        }
        .background(Theme.surface)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).strokeBorder(Theme.border))
    }
}

private struct Hero: View {
    let version: String
    /// Picked once per showing, as the title screen picks one per start.
    @State private var splash = Splash.pick()

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(spacing: 6) {
                Rectangle().fill(Theme.ready).frame(width: 5, height: 5)
                Text("MINECRAFT \(version)").font(Theme.mono(9)).foregroundStyle(Theme.text)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(Color(red: 16 / 255, green: 13 / 255, blue: 32 / 255, opacity: 0.8), in: RoundedRectangle(cornerRadius: 4))
            VStack(alignment: .leading, spacing: 6) {
                Text("Horizon").font(Theme.pixel(44)).foregroundStyle(Theme.text)
                    .overlay(alignment: .topTrailing) {
                        if let splash {
                            Splash(text: splash)
                                .offset(x: 70, y: 18)
                                .onTapGesture {
                                    Haptics.tap()
                                    self.splash = Splash.pick()
                                }
                        }
                    }
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

/// Play, in every state the game can be in: loading (the bar fills behind the word), waiting to take the
/// player somewhere (it says where, and can be cancelled), and with no game files (it says so).
private struct PlayButton: View {
    @ObservedObject var link: GameLink

    var body: some View {
        VStack(spacing: 8) {
            Button {
                Task { await link.play() }
            } label: {
                HStack(spacing: 10) {
                    if link.launching != nil {
                        ProgressView().tint(Theme.text)
                    } else {
                        Kit.image("play")
                    }
                    Text(label).font(Theme.pixel(label.count > 12 ? 20 : 27)).foregroundStyle(Theme.text)
                        .lineLimit(1).minimumScaleFactor(0.6)
                }
                .padding(.horizontal, 12)
                .frame(maxWidth: .infinity, minHeight: 54, maxHeight: 54)
                .background(alignment: .leading) {
                    GeometryReader { size in
                        ZStack(alignment: .leading) {
                            (missing ? Theme.surfaceDisabled : Theme.accent.opacity(fill < 1 ? 0.55 : 1))
                            if fill < 1 {
                                Theme.accent.frame(width: size.size.width * fill)
                            }
                        }
                    }
                }
                // The design's lit top edge: a one-point border on the top side only.
                .overlay(alignment: .top) { (missing ? Color.clear : Theme.accentEdge).frame(height: 1) }
                .clipShape(RoundedRectangle(cornerRadius: 8))
                .animation(.easeOut(duration: 0.3), value: fill)
            }
            .buttonStyle(PressStyle())
            .disabled(missing || link.launching != nil)
            if link.launching != nil {
                Button("Cancel") { link.cancelLaunch() }
                    .font(Theme.sans(12, .semiBold)).foregroundStyle(Theme.textMuted)
            }
        }
    }

    private var missing: Bool {
        if case .needsFiles = link.phase { return true }
        return false
    }

    /// How full the bar is: the loading progress, full once the game is ready.
    private var fill: Double {
        if case let .loading(progress) = link.phase { return max(progress, 0.04) }
        return 1
    }

    private var label: String {
        if let launching = link.launching { return launching }
        return missing ? "Game files needed" : "Play"
    }
}

/// The world played last, one tap away.
private struct ContinueCard: View {
    let world: World
    @ObservedObject var link: GameLink

    var body: some View {
        Button {
            Task { await link.play(world) }
        } label: {
            HStack(spacing: 12) {
                WorldIcon(world: world, size: 56)
                VStack(alignment: .leading, spacing: 4) {
                    Text("CONTINUE").font(Theme.mono(8)).foregroundStyle(Theme.accentText)
                    Text(world.name).font(Theme.pixel(19)).foregroundStyle(Theme.text).lineLimit(1)
                    Text(world.subtitle).font(Theme.sans(11)).foregroundStyle(Theme.textMuted).lineLimit(1)
                }
                Spacer(minLength: 0)
                Image(systemName: "play.circle.fill").font(.system(size: 30)).foregroundStyle(Theme.accentBright)
            }
            .padding(12)
            .card()
        }
        .buttonStyle(PressStyle())
        .disabled(link.launching != nil)
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
        .card()
    }
}
