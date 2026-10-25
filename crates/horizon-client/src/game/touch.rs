//! Touch screens (phones and tablets; Horizon's own): Pocket Edition's controls while playing, and a finger
//! as the mouse over every screen. The layout and the fingers' state are the lib's `touch`; this file acts on
//! what they say the way the keyboard and mouse would: the D-pad and buttons are their default keys, a tap is
//! a click, holding still is a held left button, and a drag turns the view as the mouse does.
use super::{Game, Screen};
use horizon_client::{
    Result,
    hud::{HudBatch, rgba},
    touch::{self, Control, Out, Touches},
};
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{
        DeviceId, ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent,
    },
    event_loop::ActiveEventLoop,
    keyboard::KeyCode,
};

/// GUI pixels a finger drags over a menu for one line of scrolling.
const SCROLL_LINE: f32 = 12.0;

#[derive(Default)]
pub(super) struct TouchInput {
    fingers: Touches,
    /// A touch has been seen: the controls are drawn (from the start on iPhone and iPad).
    seen: bool,
    /// The finger that is the pointer over a menu.
    menu: Option<MenuFinger>,
    /// Clicks to release once a tick has passed: vanilla samples buttons once a tick, so a tap released in
    /// the same frame would be lost (the pad holds its short presses back the same way).
    late: Vec<(MouseButton, u64)>,
    clock: Option<Instant>,
}

/// A finger over a menu. Its press waits for a frame drawn with the pointer under it, and its release for a
/// frame after the press: screens find what is under the pointer while drawing, so a pointer that moved and
/// pressed in one go would press whatever was under the old position (or nothing).
struct MenuFinger {
    id: u64,
    /// Where the scrolling last caught up with the finger (GUI pixels).
    scrolled: (f32, f32),
    /// Loop iterations since it went down.
    frames: u8,
    pressed: bool,
    lifted: bool,
    /// It went down on the close button, which already acted: the rest of it is ignored.
    closed: bool,
}

/// The key a button stands for: the vanilla default of its mapping.
fn key_of(control: Control) -> Option<KeyCode> {
    Some(match control {
        Control::Forward => KeyCode::KeyW,
        Control::Back => KeyCode::KeyS,
        Control::Left => KeyCode::KeyA,
        Control::Right => KeyCode::KeyD,
        Control::Jump => KeyCode::Space,
        Control::Perspective => KeyCode::F5,
        Control::Chat => KeyCode::KeyT,
        Control::Pause => KeyCode::Escape,
        Control::Inventory => KeyCode::KeyE,
        Control::Sneak | Control::Hotbar(_) => return None,
    })
}

impl Game<'_> {
    fn touch_batch(&self) -> Option<HudBatch> {
        self.renderer
            .as_ref()
            .map(|r| HudBatch::new([r.config.width, r.config.height]))
    }

    fn touch_now(&mut self) -> f64 {
        self.touch
            .clock
            .get_or_insert_with(Instant::now)
            .elapsed()
            .as_secs_f64()
    }

    /// The world is under the fingers, with no screen over it.
    fn touch_playing(&self) -> bool {
        self.title.is_none()
            && self.screen == Screen::Playing
            && self.sign_edit.is_none()
            && self.book.is_none()
            && self.player.is_some()
    }

    /// The controls are on screen: after a touch, from the start on iPhone and iPad, or with `HORIZON_TOUCH=1`
    /// (laying them out on a computer).
    fn touch_shown(&self) -> bool {
        self.touch.seen
            || cfg!(target_os = "ios")
            || std::env::var_os("HORIZON_TOUCH").is_some_and(|v| v == "1")
    }

    /// winit's `WindowEvent::Touch`.
    pub(super) fn touch_event(&mut self, event_loop: &ActiveEventLoop, touch: Touch) -> Result<()> {
        self.touch.seen = true;
        self.last_input = Instant::now();
        let Some(batch) = self.touch_batch() else {
            return Ok(());
        };
        let at = (
            touch.location.x as f32 / batch.scale,
            touch.location.y as f32 / batch.scale,
        );
        if self.touch.menu.is_some() || !self.touch_playing() {
            self.touch_menu(event_loop, touch.id, touch.phase, at);
            return Ok(());
        }
        let layout = touch::layout(batch.width, batch.height);
        let out = match touch.phase {
            TouchPhase::Started => {
                let now = self.touch_now();
                self.touch.fingers.start(touch.id, at, now, &layout)
            }
            TouchPhase::Moved => self.touch.fingers.moved(touch.id, at, &layout),
            TouchPhase::Ended | TouchPhase::Cancelled => self.touch.fingers.end(touch.id),
        };
        self.touch_act(out, batch.scale)
    }

    /// Once a frame: a held finger starts mining, held-back clicks release, and a screen that opened over the
    /// game lets go of every finger on it.
    pub(super) fn touch_frame(&mut self, event_loop: &ActiveEventLoop) {
        if !self.touch_shown() {
            return;
        }
        self.touch_menu_frame(event_loop);
        let playing = self.touch_playing();
        let out = if playing {
            let now = self.touch_now();
            self.touch.fingers.frame(now)
        } else {
            self.touch.fingers.release_all()
        };
        let ticks = self.ticks;
        let (due, keep) = std::mem::take(&mut self.touch.late)
            .into_iter()
            .partition(|(_, at)| ticks > *at || !playing);
        self.touch.late = keep;
        let scale = self.touch_batch().map_or(1.0, |b| b.scale);
        let result = due
            .into_iter()
            .try_for_each(|(button, _)| self.play_mouse(button, false))
            .and_then(|()| self.touch_act(out, scale));
        if let Err(e) = result {
            log::warn!("Horizon: touch: {e}");
        }
    }

    /// Mouse look and clicks only count with the pointer grabbed, which a touch screen has no pointer for:
    /// grab it as a click into the window would, or act as an agent's pointer where it cannot be grabbed.
    fn touch_grab(&mut self) {
        // Only when neither holds: a grab that fails (iOS has no pointer to grab) would otherwise be retried
        // on every drag, and each try makes the mouse drop its next move (`grabMouse`'s ignoreFirstMove).
        if self.captured || self.agent_pointer {
            return;
        }
        self.capture(true);
        if !self.captured {
            self.agent_pointer = true;
        }
    }

    fn touch_act(&mut self, out: Vec<Out>, scale: f32) -> Result<()> {
        for o in out {
            match o {
                Out::Press(Control::Hotbar(slot)) => self.selected = usize::from(slot),
                Out::Press(Control::Sneak) => {
                    // A toggle either way: the crouch option toggles a press, otherwise flip the held key
                    // (as a controller's stick click does).
                    if self.settings.toggle_crouch {
                        self.mapped_key(KeyCode::ShiftLeft, true, false)?;
                        self.mapped_key(KeyCode::ShiftLeft, false, false)?;
                    } else {
                        let sneaking = self.keys.contains(&KeyCode::ShiftLeft);
                        self.mapped_key(KeyCode::ShiftLeft, !sneaking, false)?;
                    }
                }
                Out::Press(control) | Out::Release(control) => {
                    if let Some(key) = key_of(control) {
                        self.mapped_key(key, matches!(o, Out::Press(_)), false)?;
                    }
                }
                Out::Look(dx, dy) => {
                    self.touch_grab();
                    let (dx, dy) = (f64::from(dx * scale), f64::from(dy * scale));
                    // Relative motion, as an agent's `mouse-move` adds it: no pointer position to compare with.
                    self.mouse.add_motion(dx, dy);
                }
                // Pocket Edition: a tap hits the entity it is on, and uses the item or block otherwise.
                Out::Tap => {
                    self.touch_grab();
                    let button = if self.entity_targeted() {
                        MouseButton::Left
                    } else {
                        MouseButton::Right
                    };
                    self.play_mouse(button, true)?;
                    self.touch.late.push((button, self.ticks));
                }
                Out::MineStart => {
                    self.touch_grab();
                    self.play_mouse(MouseButton::Left, true)?;
                }
                Out::MineEnd => self.play_mouse(MouseButton::Left, false)?,
            }
        }
        Ok(())
    }

    // ---- menus: a finger is the mouse ----

    /// The ✕ over a screen with no Done or Back of its own: an inventory, a container, creative, chat, the
    /// statistics and advancements, a sign or a book. The title screens and the options have theirs.
    fn touch_close_shown(&self) -> bool {
        self.touch_shown()
            && self.title.is_none()
            && (self.sign_edit.is_some()
                || self.book.is_some()
                || !matches!(
                    self.screen,
                    Screen::Playing | Screen::Paused | Screen::Dead | Screen::Win | Screen::Options
                ))
    }

    fn touch_menu(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: u64,
        phase: TouchPhase,
        at: (f32, f32),
    ) {
        match phase {
            TouchPhase::Started => {
                if self.touch.menu.is_some() {
                    return;
                }
                let on_close = self.touch_close_shown()
                    && self
                        .touch_batch()
                        .is_some_and(|b| touch::hit_rect(touch::close_rect(b.width, b.height), at));
                if on_close {
                    // What Escape does where the finger is (the pad's B).
                    if let Err(e) = self.pad_back() {
                        log::warn!("Horizon: touch close: {e}");
                    }
                } else {
                    self.touch_pointer(event_loop, at);
                }
                self.touch.menu = Some(MenuFinger {
                    id,
                    scrolled: at,
                    frames: 0,
                    pressed: false,
                    lifted: false,
                    closed: on_close,
                });
            }
            TouchPhase::Moved => {
                let Some(finger) = self.touch.menu.as_ref().filter(|f| f.id == id && !f.closed)
                else {
                    return;
                };
                let last = finger.scrolled;
                self.touch_pointer(event_loop, at);
                // A drag scrolls lists as a wheel would, the content following the finger. Not in an
                // inventory, where a drag carries the item it pressed on.
                let lines = ((at.1 - last.1) / SCROLL_LINE).trunc();
                let scrolls = self.screen != Screen::Inventory;
                if let Some(f) = self.touch.menu.as_mut() {
                    f.scrolled = (at.0, last.1 + lines * SCROLL_LINE);
                }
                if scrolls && lines != 0.0 {
                    self.touch_wheel(event_loop, lines);
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let Some(finger) = self.touch.menu.as_mut().filter(|f| f.id == id) else {
                    return;
                };
                finger.lifted = true;
                if !finger.closed {
                    self.touch_pointer(event_loop, at);
                }
            }
        }
    }

    /// Once a loop iteration: the menu finger's press, a frame after it went down, and its release, a frame
    /// after the press once it has lifted.
    fn touch_menu_frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some(finger) = self.touch.menu.as_mut() else {
            return;
        };
        finger.frames = finger.frames.saturating_add(1);
        if finger.closed {
            if finger.lifted {
                self.touch.menu = None;
            }
        } else if !finger.pressed {
            if finger.frames >= 2 {
                finger.pressed = true;
                self.touch_button(event_loop, true);
            }
        } else if finger.lifted {
            self.touch.menu = None;
            self.touch_button(event_loop, false);
        }
    }

    fn touch_pointer(&mut self, event_loop: &ActiveEventLoop, at: (f32, f32)) {
        let (Some(id), Some(batch)) = (self.window.as_ref().map(|w| w.id()), self.touch_batch())
        else {
            return;
        };
        let position =
            PhysicalPosition::new(f64::from(at.0 * batch.scale), f64::from(at.1 * batch.scale));
        self.window_event(
            event_loop,
            id,
            WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position,
            },
        );
    }

    fn touch_button(&mut self, event_loop: &ActiveEventLoop, pressed: bool) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        let state = if pressed {
            ElementState::Pressed
        } else {
            ElementState::Released
        };
        self.window_event(
            event_loop,
            id,
            WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state,
                button: MouseButton::Left,
            },
        );
    }

    fn touch_wheel(&mut self, event_loop: &ActiveEventLoop, lines: f32) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        self.window_event(
            event_loop,
            id,
            WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(0.0, lines),
                phase: TouchPhase::Moved,
            },
        );
    }

    // ---- drawing ----

    /// The buttons over the HUD while playing: Pocket Edition's translucent grey squares, lighter while held.
    pub(super) fn draw_touch_controls(&self, batch: &mut HudBatch) {
        if !self.touch_shown() || !self.touch_playing() {
            return;
        }
        let atlas = &self.gui.atlas;
        for (control, r) in touch::layout(batch.width, batch.height) {
            if matches!(control, Control::Hotbar(_)) {
                continue;
            }
            let held = self.touch.fingers.held(control);
            let face = if held {
                rgba(200, 200, 200, 150)
            } else {
                rgba(150, 150, 150, 110)
            };
            let edge = rgba(40, 40, 40, 140);
            batch.fill(atlas, r[0], r[1], r[2], r[3], edge);
            batch.fill(atlas, r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, face);
            glyph(batch, atlas, control, r, rgba(60, 60, 60, 220));
        }
    }
}

impl Game<'_> {
    /// The ✕ over a screen with no way out of its own on a touch screen (`touch_close_shown`).
    pub(super) fn draw_touch_close(&self, batch: &mut HudBatch) {
        if !self.touch_close_shown() {
            return;
        }
        let atlas = &self.gui.atlas;
        let r = touch::close_rect(batch.width, batch.height);
        let held = self.touch.menu.as_ref().is_some_and(|f| f.closed);
        let face = if held {
            rgba(200, 200, 200, 170)
        } else {
            rgba(150, 150, 150, 130)
        };
        batch.fill(atlas, r[0], r[1], r[2], r[3], rgba(40, 40, 40, 160));
        batch.fill(atlas, r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0, face);
        // Two diagonals of 2-pixel cells.
        let (cx, cy) = ((r[0] + r[2] / 2.0).floor(), (r[1] + r[3] / 2.0).floor());
        let ink = rgba(40, 40, 40, 230);
        for i in -3..=3 {
            let d = i as f32 * 2.0;
            batch.fill(atlas, cx + d - 1.0, cy + d - 1.0, 2.0, 2.0, ink);
            batch.fill(atlas, cx - d - 1.0, cy + d - 1.0, 2.0, 2.0, ink);
        }
    }
}

/// A button's mark, in cells of a few pixels centred on it.
fn glyph(
    batch: &mut HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    control: Control,
    r: touch::Rect,
    color: u32,
) {
    let (cx, cy) = ((r[0] + r[2] / 2.0).floor(), (r[1] + r[3] / 2.0).floor());
    // The mark's pixel: two GUI pixels on the old 26-pixel buttons, growing with them.
    let u = (r[2] / 13.0).round().max(2.0);
    let cell = |batch: &mut HudBatch, x: f32, y: f32, w: f32, h: f32| {
        batch.fill(atlas, cx + x * u, cy + y * u, w * u, h * u, color);
    };
    // A triangle pointing up, down, left or right: rows (or columns) narrowing to its tip.
    let arrow = |batch: &mut HudBatch, dir: (f32, f32)| {
        for i in 0..4 {
            let i = i as f32;
            let len = 7.0 - 2.0 * i;
            let at = 1.5 - i;
            match dir {
                (0.0, d) => cell(batch, -len / 2.0, at * -d - 0.5, len, 1.0),
                (d, _) => cell(batch, at * -d - 0.5, -len / 2.0, 1.0, len),
            }
        }
    };
    match control {
        Control::Forward => arrow(batch, (0.0, -1.0)),
        Control::Back => arrow(batch, (0.0, 1.0)),
        Control::Left => arrow(batch, (-1.0, 0.0)),
        Control::Right => arrow(batch, (1.0, 0.0)),
        // Sneak: a hollow diamond; jump: a filled one.
        Control::Sneak | Control::Jump => {
            for i in 0..4 {
                let i = i as f32;
                let len = 7.0 - 2.0 * i;
                for y in [-i - 0.5, i - 0.5] {
                    if control == Control::Jump {
                        cell(batch, -len / 2.0, y, len, 1.0);
                    } else {
                        cell(batch, -len / 2.0, y, 1.0, 1.0);
                        cell(batch, len / 2.0 - 1.0, y, 1.0, 1.0);
                    }
                }
            }
        }
        // Perspective: an eye.
        Control::Perspective => {
            cell(batch, -2.5, -1.5, 5.0, 1.0);
            cell(batch, -3.5, -0.5, 1.0, 1.0);
            cell(batch, -0.5, -0.5, 1.0, 1.0);
            cell(batch, 2.5, -0.5, 1.0, 1.0);
            cell(batch, -2.5, 0.5, 5.0, 1.0);
        }
        // Chat: a speech bubble with its tail.
        Control::Chat => {
            cell(batch, -3.0, -2.5, 6.0, 4.0);
            cell(batch, -2.0, 1.5, 1.0, 1.0);
        }
        // Pause: two bars.
        Control::Pause => {
            cell(batch, -2.0, -2.5, 1.5, 5.0);
            cell(batch, 0.5, -2.5, 1.5, 5.0);
        }
        // The inventory: three dots, as Pocket Edition's button beside the hotbar.
        Control::Inventory => {
            for x in [-3.0, -0.5, 2.0] {
                cell(batch, x, -0.5, 1.0, 1.0);
            }
        }
        Control::Hotbar(_) => {}
    }
}
