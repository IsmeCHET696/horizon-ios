//! Controllers on iPhone and iPad: Apple's GameController framework, which pairs and reads every pad iOS
//! supports (DualSense, DualShock 4, Xbox, MFi) where gilrs has no backend. Read only: no rumble, lights or
//! adaptive triggers yet, so the output half is the same no-op as a build without controllers.
use super::{super::Game, Pads};
use horizon_client::gamepad::{Button, GlyphStyle, PadState, Pulse, Rumble};
use objc2::rc::Retained;
use objc2_game_controller::{GCController, GCControllerButtonInput, GCDevice, GCExtendedGamepad};
use serde_json::{Value, json};

/// How many controllers the last poll saw, to log connections the way gilrs' events do.
#[derive(Default)]
pub(super) struct Devices {
    connected: usize,
}

fn pressed(button: &GCControllerButtonInput) -> bool {
    unsafe { button.isPressed() }
}

/// The pad that drives the game: the one iOS last saw used, else the first with a full gamepad profile.
fn active() -> Option<(Retained<GCController>, Retained<GCExtendedGamepad>)> {
    unsafe {
        let current = GCController::current().filter(|c| c.extendedGamepad().is_some());
        let controller = current.or_else(|| {
            GCController::controllers()
                .iter()
                .find(|c| c.extendedGamepad().is_some())
        })?;
        let pad = controller.extendedGamepad()?;
        Some((controller, pad))
    }
}

/// The glyphs to draw, from the product category iOS reports ("DualSense", "Xbox One", "Switch Pro
/// Controller"...), through the same vendor ids gilrs' pads are matched by.
fn style(controller: &GCController) -> GlyphStyle {
    let category = unsafe { controller.productCategory() }.to_string();
    let vendor = if category.contains("DualSense") || category.contains("DualShock") {
        Some(0x054c)
    } else if category.contains("Switch") || category.contains("Joy-Con") {
        Some(0x057e)
    } else {
        None
    };
    GlyphStyle::of_vendor(vendor)
}

impl Pads {
    /// Read the active pad. GameController keeps its state current on the main thread's run loop, which is
    /// the one winit's loop runs, so a poll is a read of properties and needs no event queue.
    pub(super) fn read(&mut self) -> PadState {
        let controllers = unsafe { GCController::controllers() };
        if controllers.count() != self.devices.connected {
            for controller in controllers.iter() {
                let name =
                    unsafe { controller.vendorName() }.map_or_else(String::new, |n| n.to_string());
                log::info!("Controller connected: {name}");
            }
            self.devices.connected = controllers.count();
        }
        let Some((controller, pad)) = active() else {
            return PadState::default();
        };
        self.style = style(&controller);
        let mut state = PadState::default();
        unsafe {
            let dpad = pad.dpad();
            let buttons = [
                (Button::South, Some(pad.buttonA())),
                (Button::East, Some(pad.buttonB())),
                (Button::West, Some(pad.buttonX())),
                (Button::North, Some(pad.buttonY())),
                (Button::LeftBumper, Some(pad.leftShoulder())),
                (Button::RightBumper, Some(pad.rightShoulder())),
                (Button::Select, pad.buttonOptions()),
                (Button::Start, Some(pad.buttonMenu())),
                (Button::LeftStick, pad.leftThumbstickButton()),
                (Button::RightStick, pad.rightThumbstickButton()),
                (Button::DPadUp, Some(dpad.up())),
                (Button::DPadDown, Some(dpad.down())),
                (Button::DPadLeft, Some(dpad.left())),
                (Button::DPadRight, Some(dpad.right())),
            ];
            for (button, input) in buttons {
                state.set(button, input.is_some_and(|b| pressed(&b)));
            }
            // GameController's sticks are -1..1 with up positive, as gilrs' are.
            let (left, right) = (pad.leftThumbstick(), pad.rightThumbstick());
            state.left = [left.xAxis().value(), left.yAxis().value()];
            state.right = [right.xAxis().value(), right.yAxis().value()];
            state.triggers = [pad.leftTrigger().value(), pad.rightTrigger().value()];
        }
        state
    }
}

impl Game<'_> {
    pub(super) fn pad_output_off(&mut self) {}
    pub(super) fn pad_output_frame(&mut self) {}
    pub(in crate::game) fn pad_close_output(&mut self) {}
    pub(super) fn pad_vibrate(&mut self, _: Rumble, _: Pulse) {}
    pub(super) fn pad_devices_status(&self) -> Value {
        let pads: Vec<Value> = unsafe { GCController::controllers() }
            .iter()
            .map(|c| {
                json!({
                    "name": unsafe { c.vendorName() }.map(|n| n.to_string()),
                    "category": unsafe { c.productCategory() }.to_string(),
                    "gamepad": unsafe { c.extendedGamepad() }.is_some(),
                })
            })
            .collect();
        json!({ "backend": true, "controllers": pads })
    }
}
