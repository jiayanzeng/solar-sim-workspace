//! WP5 — the sole raw-device-input boundary (ARCHITECTURE invariant 4).
//!
//! Raw Bevy events become semantic `InputIntent`s here, then a second system
//! translates each intent into exactly one `SimCommand`. No other module reads
//! keyboard or mouse state; future UI widgets join at the command queue seam.

use crate::control::{SimCommand, SimCommandQueue};
use crate::SimulationSet;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use sim_core::time::RateIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyIntent {
    Travel(&'static str),
    StepRate(i8),
    SetRate(RateIndex),
    Play,
    Pause,
    TogglePlay,
}

#[derive(Debug, Clone, PartialEq)]
enum InputIntent {
    Key(KeyIntent),
    Orbit { delta_yaw: f64, delta_pitch: f64 },
    Dolly { delta: f64 },
}

#[derive(Debug, Clone, Copy)]
struct KeyBinding {
    key: KeyCode,
    intent: KeyIntent,
}

const KEY_BINDINGS: &[KeyBinding] = &[
    KeyBinding {
        key: KeyCode::KeyO,
        intent: KeyIntent::Travel("sun"),
    },
    KeyBinding {
        key: KeyCode::KeyM,
        intent: KeyIntent::Travel("mercury"),
    },
    KeyBinding {
        key: KeyCode::KeyS,
        intent: KeyIntent::Travel("sedna"),
    },
    KeyBinding {
        key: KeyCode::KeyI,
        intent: KeyIntent::Travel("io"),
    },
    KeyBinding {
        key: KeyCode::BracketLeft,
        intent: KeyIntent::StepRate(-1),
    },
    KeyBinding {
        key: KeyCode::BracketRight,
        intent: KeyIntent::StepRate(1),
    },
    KeyBinding {
        key: KeyCode::Digit1,
        intent: KeyIntent::SetRate(RateIndex::REAL),
    },
    KeyBinding {
        key: KeyCode::KeyR,
        intent: KeyIntent::Play,
    },
    KeyBinding {
        key: KeyCode::KeyP,
        intent: KeyIntent::Pause,
    },
    KeyBinding {
        key: KeyCode::Space,
        intent: KeyIntent::TogglePlay,
    },
];

#[derive(Resource, Default)]
struct InputIntentQueue(Vec<InputIntent>);

pub(crate) struct InputIntentPlugin;

impl Plugin for InputIntentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputIntentQueue>().add_systems(
            Update,
            (collect_raw_intents, translate_intents)
                .chain()
                .in_set(SimulationSet::Input),
        );
    }
}

fn collect_raw_intents(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut intents: ResMut<InputIntentQueue>,
) {
    if buttons.pressed(MouseButton::Right) {
        for event in motion.read() {
            intents.0.push(InputIntent::Orbit {
                delta_yaw: f64::from(event.delta.x),
                delta_pitch: f64::from(event.delta.y),
            });
        }
    } else {
        motion.clear();
    }
    for event in wheel.read() {
        intents.0.push(InputIntent::Dolly {
            delta: f64::from(event.y),
        });
    }
    for binding in KEY_BINDINGS {
        if keys.just_pressed(binding.key) {
            intents.0.push(InputIntent::Key(binding.intent));
        }
    }
}

fn translate_intents(mut intents: ResMut<InputIntentQueue>, mut commands: ResMut<SimCommandQueue>) {
    for intent in intents.0.drain(..) {
        commands.push(intent_to_command(intent));
    }
}

fn intent_to_command(intent: InputIntent) -> SimCommand {
    match intent {
        InputIntent::Key(KeyIntent::Travel(id)) => SimCommand::TravelToBody(id.into()),
        InputIntent::Key(KeyIntent::StepRate(delta)) => SimCommand::StepRate(delta),
        InputIntent::Key(KeyIntent::SetRate(rate)) => SimCommand::SetRate(rate),
        InputIntent::Key(KeyIntent::Play) => SimCommand::Play,
        InputIntent::Key(KeyIntent::Pause) => SimCommand::Pause,
        InputIntent::Key(KeyIntent::TogglePlay) => SimCommand::TogglePlay,
        InputIntent::Orbit {
            delta_yaw,
            delta_pitch,
        } => SimCommand::Orbit {
            delta_yaw,
            delta_pitch,
        },
        InputIntent::Dolly { delta } => dolly_command(delta),
    }
}

pub(crate) fn dolly_command(delta: f64) -> SimCommand {
    SimCommand::Dolly { delta }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ZOOM_IN_DOLLY_DELTA, ZOOM_OUT_DOLLY_DELTA};
    use std::collections::HashSet;

    #[test]
    fn every_bound_key_produces_exactly_one_command() {
        let mut unique_keys = HashSet::new();
        for binding in KEY_BINDINGS {
            assert!(
                unique_keys.insert(binding.key),
                "duplicate binding for {:?}",
                binding.key
            );
            let matches: Vec<_> = KEY_BINDINGS
                .iter()
                .filter(|candidate| candidate.key == binding.key)
                .map(|candidate| intent_to_command(InputIntent::Key(candidate.intent)))
                .collect();
            assert_eq!(matches.len(), 1, "{:?}", binding.key);
        }
    }

    #[test]
    fn rail_zoom_and_unit_scroll_use_identical_dolly_commands() {
        assert_eq!(
            intent_to_command(InputIntent::Dolly {
                delta: ZOOM_IN_DOLLY_DELTA,
            }),
            dolly_command(ZOOM_IN_DOLLY_DELTA)
        );
        assert_eq!(
            intent_to_command(InputIntent::Dolly {
                delta: ZOOM_OUT_DOLLY_DELTA,
            }),
            dolly_command(ZOOM_OUT_DOLLY_DELTA)
        );
    }
}
