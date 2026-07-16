//! WP5 — the sole raw-device-input boundary (ARCHITECTURE invariant 4).
//!
//! Raw Bevy events become semantic `InputIntent`s here, then a second system
//! translates each intent into exactly one `SimCommand`. No other module reads
//! keyboard or mouse state; future UI widgets join at the command queue seam.

use crate::control::{SimCommand, SimCommandQueue};
use crate::search::BrowseUiState;
use crate::settings::SettingsScreenState;
use crate::{AppSettings, SimulationSet};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::input_focus::InputFocus;
use bevy::picking::hover::HoverMap;
use bevy::prelude::*;
use bevy::text::EditableText;
use sim_core::time::RateIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyIntent {
    Travel(&'static str),
    StepRate(i8),
    SetRate(RateIndex),
    Play,
    Pause,
    TogglePlay,
    CloseSettings,
    CloseBrowse,
    #[cfg(debug_assertions)]
    SimulateDeviceLoss,
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
    #[cfg(debug_assertions)]
    KeyBinding {
        key: KeyCode::F9,
        intent: KeyIntent::SimulateDeviceLoss,
    },
];

#[derive(Resource, Default)]
struct InputIntentQueue(Vec<InputIntent>);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum InteractionContext {
    #[default]
    Gameplay,
    TextEdit,
    BrowseModal,
    SettingsModal,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct InteractionState {
    context: InteractionContext,
    pointer_over_scroll_surface: bool,
}

impl InteractionState {
    pub(crate) const fn context(&self) -> InteractionContext {
        self.context
    }

    pub(crate) const fn blocks_gameplay(&self) -> bool {
        !matches!(self.context, InteractionContext::Gameplay)
    }

    pub(crate) const fn captures_wheel(&self) -> bool {
        self.blocks_gameplay() || self.pointer_over_scroll_surface
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct UiScrollSurface;

pub(crate) struct InputIntentPlugin;

impl Plugin for InputIntentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputIntentQueue>()
            .init_resource::<InteractionState>()
            .add_systems(
                Update,
                (
                    sync_interaction_context,
                    collect_raw_intents,
                    translate_intents,
                )
                    .chain()
                    .in_set(SimulationSet::Input),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_interaction_context(
    focus: Res<InputFocus>,
    editable: Query<(), With<EditableText>>,
    settings_screen: Res<SettingsScreenState>,
    browse: Res<BrowseUiState>,
    hover_map: Res<HoverMap>,
    scroll_surfaces: Query<(), With<UiScrollSurface>>,
    parents: Query<&ChildOf>,
    mut interaction: ResMut<InteractionState>,
) {
    interaction.context = if settings_screen.is_open() {
        InteractionContext::SettingsModal
    } else if browse.is_open() {
        InteractionContext::BrowseModal
    } else if focus
        .get()
        .is_some_and(|entity| editable.get(entity).is_ok())
    {
        InteractionContext::TextEdit
    } else {
        InteractionContext::Gameplay
    };
    interaction.pointer_over_scroll_surface = hover_map.values().any(|hits| {
        hits.keys()
            .copied()
            .any(|entity| is_within_scroll_surface(entity, &scroll_surfaces, &parents))
    });
}

fn is_within_scroll_surface(
    mut entity: Entity,
    scroll_surfaces: &Query<(), With<UiScrollSurface>>,
    parents: &Query<&ChildOf>,
) -> bool {
    for _ in 0..16 {
        if scroll_surfaces.get(entity).is_ok() {
            return true;
        }
        let Ok(parent) = parents.get(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}

fn collect_raw_intents(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mut intents: ResMut<InputIntentQueue>,
    interaction: Res<InteractionState>,
) {
    match interaction.context() {
        InteractionContext::SettingsModal => {
            if keys.just_pressed(KeyCode::Escape) {
                intents.0.push(InputIntent::Key(KeyIntent::CloseSettings));
            }
        }
        InteractionContext::BrowseModal => {
            if keys.just_pressed(KeyCode::Escape) {
                intents.0.push(InputIntent::Key(KeyIntent::CloseBrowse));
            }
        }
        InteractionContext::TextEdit => {}
        InteractionContext::Gameplay => {
            for binding in KEY_BINDINGS {
                if keys.just_pressed(binding.key) {
                    intents.0.push(InputIntent::Key(binding.intent));
                }
            }
        }
    }
    if interaction.blocks_gameplay() {
        motion.clear();
        wheel.clear();
        return;
    }
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
    if interaction.captures_wheel() {
        wheel.clear();
    } else {
        for event in wheel.read() {
            intents.0.push(InputIntent::Dolly {
                delta: f64::from(event.y),
            });
        }
    }
}

fn translate_intents(
    mut intents: ResMut<InputIntentQueue>,
    settings: Res<AppSettings>,
    mut commands: ResMut<SimCommandQueue>,
) {
    for intent in intents.0.drain(..) {
        commands.push(apply_axis_inversion(
            intent_to_command(intent),
            settings.invert_horizontal,
            settings.invert_vertical,
        ));
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
        InputIntent::Key(KeyIntent::CloseSettings) => SimCommand::CloseSettings,
        InputIntent::Key(KeyIntent::CloseBrowse) => SimCommand::SetBrowseOpen(false),
        #[cfg(debug_assertions)]
        InputIntent::Key(KeyIntent::SimulateDeviceLoss) => SimCommand::SimulateDeviceLoss,
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

fn apply_axis_inversion(
    command: SimCommand,
    invert_horizontal: bool,
    invert_vertical: bool,
) -> SimCommand {
    match command {
        SimCommand::Orbit {
            delta_yaw,
            delta_pitch,
        } => SimCommand::Orbit {
            delta_yaw: if invert_horizontal {
                -delta_yaw
            } else {
                delta_yaw
            },
            delta_pitch: if invert_vertical {
                -delta_pitch
            } else {
                delta_pitch
            },
        },
        command => command,
    }
}

pub(crate) fn dolly_command(delta: f64) -> SimCommand {
    SimCommand::Dolly { delta }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::consume_search_command;
    use crate::{ZOOM_IN_DOLLY_DELTA, ZOOM_OUT_DOLLY_DELTA};
    use bevy::input_focus::FocusCause;
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
    fn modal_and_text_contexts_block_gameplay_and_escape_has_one_owner() {
        for context in [
            InteractionContext::TextEdit,
            InteractionContext::BrowseModal,
            InteractionContext::SettingsModal,
        ] {
            assert!(InteractionState {
                context,
                ..default()
            }
            .blocks_gameplay());
        }
        assert!(!InteractionState {
            context: InteractionContext::Gameplay,
            ..default()
        }
        .blocks_gameplay());
        assert_eq!(
            intent_to_command(InputIntent::Key(KeyIntent::CloseSettings)),
            SimCommand::CloseSettings
        );
        assert_eq!(
            intent_to_command(InputIntent::Key(KeyIntent::CloseBrowse)),
            SimCommand::SetBrowseOpen(false)
        );
        assert!(!KEY_BINDINGS
            .iter()
            .any(|binding| binding.key == KeyCode::Escape));
    }

    fn interaction_test_app(browse_open: bool) -> App {
        let mut browse = BrowseUiState::default();
        if browse_open {
            consume_search_command(&SimCommand::SetBrowseOpen(true), &mut browse);
        }
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<MouseMotion>()
            .add_message::<MouseWheel>()
            .init_resource::<InputFocus>()
            .init_resource::<HoverMap>()
            .insert_resource(SettingsScreenState::default())
            .insert_resource(browse)
            .insert_resource(AppSettings::default())
            .init_resource::<SimCommandQueue>()
            .add_plugins(InputIntentPlugin);
        app
    }

    #[test]
    fn focused_editable_text_blocks_every_gameplay_hotkey() {
        let mut app = interaction_test_app(false);
        let editable = app.world_mut().spawn(EditableText::new("")).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(editable, FocusCause::Navigated);
        for key in [
            KeyCode::KeyS,
            KeyCode::KeyM,
            KeyCode::KeyI,
            KeyCode::KeyO,
            KeyCode::KeyR,
            KeyCode::KeyP,
            KeyCode::Space,
            KeyCode::Digit1,
            KeyCode::BracketLeft,
            KeyCode::BracketRight,
        ] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
        }
        app.update();
        assert_eq!(
            app.world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .count(),
            0
        );
        assert_eq!(
            app.world().resource::<InteractionState>().context(),
            InteractionContext::TextEdit
        );
    }

    #[test]
    fn browse_modal_blocks_hotkeys_and_escape_only_closes_browse() {
        let mut app = interaction_test_app(true);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyS);
            keys.press(KeyCode::Space);
            keys.press(KeyCode::Escape);
        }
        app.update();
        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::SetBrowseOpen(false)]);
    }

    #[test]
    fn browse_modal_discards_right_drag_and_wheel_gameplay_input() {
        let mut app = interaction_test_app(true);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(8.0, -3.0),
        });
        app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });

        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .count(),
            0
        );
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

    #[test]
    fn axis_preferences_only_invert_orbit_command_components() {
        let orbit = SimCommand::Orbit {
            delta_yaw: 4.0,
            delta_pitch: -2.0,
        };
        assert_eq!(
            apply_axis_inversion(orbit.clone(), true, false),
            SimCommand::Orbit {
                delta_yaw: -4.0,
                delta_pitch: -2.0,
            }
        );
        assert_eq!(
            apply_axis_inversion(orbit, false, true),
            SimCommand::Orbit {
                delta_yaw: 4.0,
                delta_pitch: 2.0,
            }
        );
        assert_eq!(
            apply_axis_inversion(SimCommand::Play, true, true),
            SimCommand::Play
        );
    }
}
