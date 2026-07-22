//! WP5/UIP-5 — the sole raw-device-input boundary (ARCHITECTURE invariant 4).
//!
//! Raw Bevy events become semantic `InputIntent`s here, then a second system
//! translates each intent into exactly one `SimCommand`. No other module reads
//! keyboard or mouse state; future UI widgets join at the command queue seam.

use crate::control::{RegionPreset, SimCommand, SimCommandQueue};
use crate::help::HelpModalRoot;
use crate::layers::HudSurface;
use crate::search::{BrowseMenuRoot, BrowseUiState, SearchDropdownRoot};
use crate::settings::SettingsScreenRoot;
use crate::{
    AppSettings, LayerId, LayerState, PresentationState, SimulationSet, UiRestoreAffordance,
};
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::input::InputSystems;
use bevy::input_focus::{InputFocus, InputFocusSystems};
use bevy::picking::{hover::HoverMap, PickingSystems};
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::ScrollIntoView;
use bevy::{
    ecs::system::SystemParam,
    input_focus::{tab_navigation::TabIndex, FocusCause},
    ui_widgets::Button,
};
use sim_core::time::RateIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyIntent {
    Travel(&'static str),
    TravelRegion(RegionPreset),
    StepRate(i8),
    Play,
    Pause,
    TogglePlay,
    SetDayRate,
    ResetView,
    OpenHelp,
    CloseHelp,
    CloseSettings,
    CloseBrowse,
    #[cfg(debug_assertions)]
    SimulateDeviceLoss,
    #[cfg(debug_assertions)]
    ToggleDiagnosticsOverlay,
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
        key: KeyCode::ArrowLeft,
        intent: KeyIntent::StepRate(-1),
    },
    KeyBinding {
        key: KeyCode::BracketRight,
        intent: KeyIntent::StepRate(1),
    },
    KeyBinding {
        key: KeyCode::ArrowRight,
        intent: KeyIntent::StepRate(1),
    },
    KeyBinding {
        key: KeyCode::ArrowDown,
        intent: KeyIntent::SetDayRate,
    },
    KeyBinding {
        key: KeyCode::Digit1,
        intent: KeyIntent::TravelRegion(RegionPreset::Inner),
    },
    KeyBinding {
        key: KeyCode::Digit2,
        intent: KeyIntent::TravelRegion(RegionPreset::Belt),
    },
    KeyBinding {
        key: KeyCode::Digit3,
        intent: KeyIntent::TravelRegion(RegionPreset::Outer),
    },
    KeyBinding {
        key: KeyCode::Digit4,
        intent: KeyIntent::TravelRegion(RegionPreset::Kuiper),
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
    KeyBinding {
        key: KeyCode::Home,
        intent: KeyIntent::ResetView,
    },
    #[cfg(debug_assertions)]
    KeyBinding {
        key: KeyCode::F9,
        intent: KeyIntent::SimulateDeviceLoss,
    },
    #[cfg(debug_assertions)]
    KeyBinding {
        key: KeyCode::F10,
        intent: KeyIntent::ToggleDiagnosticsOverlay,
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
    HelpModal,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct InteractionState {
    // Latched before keyboard and picking dispatch so a focus-clearing handler
    // cannot release gameplay input during the remainder of the same frame.
    context: InteractionContext,
    focused_widget_owns_activation: bool,
}

#[derive(Resource, Debug, Default)]
struct PointerCaptureState {
    // Pointer hover is routing state, not a competing interaction context.
    pointer_over_scroll_surface: bool,
    pointer_over_hud_surface: bool,
}

/// Primary-pointer gesture latch shared with label and viewport activation.
/// Bevy emits `Click` before `DragEnd`, so the latch remains true precisely
/// long enough to keep a threshold-crossing orbit gesture from also selecting.
#[derive(Resource, Debug, Default)]
pub(crate) struct PrimaryDragState {
    crossed_threshold: bool,
}

impl PrimaryDragState {
    pub(crate) const fn blocks_click(&self) -> bool {
        self.crossed_threshold
    }

    #[cfg(test)]
    pub(crate) const fn crossed() -> Self {
        Self {
            crossed_threshold: true,
        }
    }
}

const PRIMARY_DRAG_THRESHOLD_PX: f32 = 5.0;

#[cfg(test)]
impl InteractionState {
    pub(crate) const fn for_context(context: InteractionContext) -> Self {
        Self {
            context,
            focused_widget_owns_activation: false,
        }
    }
}

#[derive(SystemParam)]
struct InteractionInputs<'w, 's> {
    focus: Res<'w, InputFocus>,
    editable: Query<'w, 's, (), With<EditableText>>,
    search_dropdowns: Query<'w, 's, Entity, With<SearchDropdownRoot>>,
    parents: Query<'w, 's, &'static ChildOf>,
    browse: Res<'w, BrowseUiState>,
    presentation: Res<'w, PresentationState>,
    widget_buttons: Query<'w, 's, (), With<Button>>,
}

impl InteractionInputs<'_, '_> {
    fn context(&self) -> InteractionContext {
        let focused_editable = self
            .focus
            .get()
            .is_some_and(|entity| self.editable.get(entity).is_ok());
        let focused_search_dropdown = self.focus.get().is_some_and(|entity| {
            self.search_dropdowns
                .iter()
                .any(|root| is_descendant_of(entity, root, &self.parents))
        });
        resolve_interaction_context(
            focused_editable || focused_search_dropdown,
            self.browse.is_open(),
            self.presentation.is_settings_open(),
            self.presentation.is_help_open(),
        )
    }

    fn focused_widget_owns_activation(&self) -> bool {
        self.focus
            .get()
            .is_some_and(|entity| self.widget_buttons.get(entity).is_ok())
    }
}

#[derive(SystemParam)]
pub(crate) struct InteractionOwnership<'w, 's> {
    current: InteractionInputs<'w, 's>,
    state: Option<Res<'w, InteractionState>>,
    primary_drag: Option<Res<'w, PrimaryDragState>>,
}

impl InteractionOwnership<'_, '_> {
    fn context(&self) -> InteractionContext {
        let claimed = self
            .state
            .as_deref()
            .map_or(InteractionContext::Gameplay, |state| state.context);
        if matches!(claimed, InteractionContext::Gameplay) {
            self.current.context()
        } else {
            claimed
        }
    }

    pub(crate) fn blocks_gameplay(&self) -> bool {
        !matches!(self.context(), InteractionContext::Gameplay)
    }

    pub(crate) fn blocks_primary_click(&self) -> bool {
        self.blocks_gameplay()
            || self
                .primary_drag
                .as_deref()
                .is_some_and(PrimaryDragState::blocks_click)
    }

    fn focused_widget_owns_activation(&self) -> bool {
        self.state
            .as_deref()
            .is_some_and(|state| state.focused_widget_owns_activation)
            || self.current.focused_widget_owns_activation()
    }
}

impl InteractionState {
    #[cfg(test)]
    pub(crate) const fn context(&self) -> InteractionContext {
        self.context
    }

    #[cfg(test)]
    pub(crate) const fn blocks_gameplay(&self) -> bool {
        !matches!(self.context, InteractionContext::Gameplay)
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct UiScrollSurface;

pub(crate) struct InputIntentPlugin;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ModalSurfaceSet {
    Rebuild,
    Focus,
}

impl Plugin for InputIntentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputIntentQueue>()
            .init_resource::<InteractionState>()
            .init_resource::<PointerCaptureState>()
            .init_resource::<PrimaryDragState>()
            .configure_sets(
                Update,
                (ModalSurfaceSet::Rebuild, ModalSurfaceSet::Focus)
                    .chain()
                    .in_set(SimulationSet::Render),
            )
            .add_systems(
                PreUpdate,
                latch_interaction_state
                    .after(InputSystems)
                    .before(InputFocusSystems::Dispatch)
                    .before(PickingSystems::ProcessInput),
            )
            .add_systems(
                Update,
                (sync_pointer_capture, collect_raw_intents, translate_intents)
                    .chain()
                    .in_set(SimulationSet::Input),
            )
            .add_systems(Update, reconcile_modal_focus.in_set(ModalSurfaceSet::Focus))
            .add_systems(
                Update,
                ensure_focused_control_visible
                    .after(ModalSurfaceSet::Focus)
                    .in_set(SimulationSet::Render),
            )
            .add_observer(scroll_registered_surface_into_view)
            .add_observer(begin_primary_drag)
            .add_observer(collect_primary_drag)
            .add_observer(finish_primary_drag);
    }
}

fn latch_interaction_state(inputs: InteractionInputs, mut state: ResMut<InteractionState>) {
    let context = inputs.context();
    let focused_widget_owns_activation = inputs.focused_widget_owns_activation();
    if state.context != context {
        state.context = context;
    }
    if state.focused_widget_owns_activation != focused_widget_owns_activation {
        state.focused_widget_owns_activation = focused_widget_owns_activation;
    }
}

#[derive(SystemParam)]
struct ModalFocusParams<'w, 's> {
    browse: Res<'w, BrowseUiState>,
    presentation: Res<'w, PresentationState>,
    layers: Option<Res<'w, LayerState>>,
    restore_affordances: Query<'w, 's, Entity, With<UiRestoreAffordance>>,
    browse_roots: Query<'w, 's, Entity, With<BrowseMenuRoot>>,
    settings_roots: Query<'w, 's, Entity, With<SettingsScreenRoot>>,
    help_roots: Query<'w, 's, Entity, With<HelpModalRoot>>,
    tab_indices: Query<'w, 's, (Entity, &'static TabIndex)>,
    parents: Query<'w, 's, &'static ChildOf>,
    focus: ResMut<'w, InputFocus>,
}

fn reconcile_modal_focus(params: ModalFocusParams) {
    let ModalFocusParams {
        browse,
        presentation,
        layers,
        restore_affordances,
        browse_roots,
        settings_roots,
        help_roots,
        tab_indices,
        parents,
        mut focus,
    } = params;
    // UI-off retains the normal HUD entities in a hidden state. Do not let
    // those retained Browse/Settings tab groups steal focus from the sole
    // reachable recovery control on the following frame.
    if layers
        .as_deref()
        .is_some_and(|layers| !layers.is_visible(LayerId::UserInterface))
    {
        if let Ok(restore) = restore_affordances.single() {
            if focus.get() != Some(restore) {
                focus.set(restore, FocusCause::Navigated);
            }
        } else {
            focus.clear();
        }
        return;
    }
    let active_root = if browse.is_open() {
        browse_roots.single().ok()
    } else if presentation.is_settings_open() {
        settings_roots.single().ok()
    } else if presentation.is_help_open() {
        help_roots.single().ok()
    } else {
        None
    };
    let Some(active_root) = active_root else {
        return;
    };
    if focus
        .get()
        .is_some_and(|entity| is_descendant_of(entity, active_root, &parents))
    {
        return;
    }
    let next = tab_indices
        .iter()
        .filter(|(entity, index)| index.0 >= 0 && is_descendant_of(*entity, active_root, &parents))
        .min_by_key(|(entity, index)| (index.0, entity.to_bits()))
        .map(|(entity, _)| entity);
    if let Some(next) = next {
        focus.set(next, FocusCause::Navigated);
    }
}

fn is_descendant_of(mut entity: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    for _ in 0..32 {
        if entity == ancestor {
            return true;
        }
        let Ok(parent) = parents.get(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}

fn ensure_focused_control_visible(
    focus: Res<InputFocus>,
    mut previous: Local<Option<Entity>>,
    mut commands: Commands,
) {
    let focused = focus.get();
    if *previous == focused {
        return;
    }
    *previous = focused;
    if let Some(entity) = focused {
        commands.trigger(ScrollIntoView { entity });
    }
}

fn scroll_registered_surface_into_view(
    mut scroll: On<ScrollIntoView>,
    nodes: Query<(&Node, &UiGlobalTransform, &ComputedNode)>,
    parents: Query<&ChildOf>,
    mut surfaces: Query<&mut ScrollPosition, With<UiScrollSurface>>,
) {
    let Ok((_target_node, target_transform, target_node)) = nodes.get(scroll.entity) else {
        return;
    };
    let Some(surface_entity) = parents
        .iter_ancestors(scroll.entity)
        .find(|entity| surfaces.contains(*entity))
    else {
        return;
    };
    let Ok((surface_node, surface_transform, surface_computed)) = nodes.get(surface_entity) else {
        return;
    };
    let Ok(mut position) = surfaces.get_mut(surface_entity) else {
        return;
    };

    let target_size = target_node.size() * target_node.inverse_scale_factor;
    let target_top_left = target_transform.affine().translation * target_node.inverse_scale_factor
        - target_size * 0.5;
    let surface_size = surface_computed.size() * surface_computed.inverse_scale_factor;
    let surface_top_left = surface_transform.affine().translation
        * surface_computed.inverse_scale_factor
        - surface_size * 0.5;
    let target_local_min = target_top_left - surface_top_left + position.0;
    let target_local_max = target_local_min + target_size;
    let content_size = surface_computed.content_size() * surface_computed.inverse_scale_factor;
    let max_range = (content_size - surface_size).max(Vec2::ZERO);

    if surface_node.overflow.x == OverflowAxis::Scroll {
        if target_local_min.x < position.x {
            position.x = target_local_min.x.clamp(0.0, max_range.x);
        } else if target_local_max.x > position.x + surface_size.x {
            position.x = (target_local_max.x - surface_size.x).clamp(0.0, max_range.x);
        }
    }
    if surface_node.overflow.y == OverflowAxis::Scroll {
        if target_local_min.y < position.y {
            position.y = target_local_min.y.clamp(0.0, max_range.y);
        } else if target_local_max.y > position.y + surface_size.y {
            position.y = (target_local_max.y - surface_size.y).clamp(0.0, max_range.y);
        }
    }
    scroll.propagate(false);
}

fn sync_pointer_capture(
    hover_map: Res<HoverMap>,
    scroll_surfaces: Query<(), With<UiScrollSurface>>,
    hud_surfaces: Query<Entity, With<HudSurface>>,
    parents: Query<&ChildOf>,
    mut capture: ResMut<PointerCaptureState>,
) {
    let pointer_over_scroll_surface = hover_map.values().any(|hits| {
        hits.keys()
            .copied()
            .any(|entity| is_within_scroll_surface(entity, &scroll_surfaces, &parents))
    });
    let pointer_over_hud_surface = hover_map.values().any(|hits| {
        hits.keys().copied().any(|entity| {
            hud_surfaces
                .iter()
                .any(|root| is_descendant_of(entity, root, &parents))
        })
    });
    if capture.pointer_over_scroll_surface != pointer_over_scroll_surface
        || capture.pointer_over_hud_surface != pointer_over_hud_surface
    {
        capture.pointer_over_scroll_surface = pointer_over_scroll_surface;
        capture.pointer_over_hud_surface = pointer_over_hud_surface;
    }
}

const fn resolve_interaction_context(
    focused_editable: bool,
    browse_open: bool,
    settings_open: bool,
    help_open: bool,
) -> InteractionContext {
    if focused_editable {
        InteractionContext::TextEdit
    } else if browse_open {
        InteractionContext::BrowseModal
    } else if settings_open {
        InteractionContext::SettingsModal
    } else if help_open {
        InteractionContext::HelpModal
    } else {
        InteractionContext::Gameplay
    }
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
    capture: Res<PointerCaptureState>,
    ownership: InteractionOwnership,
) {
    match ownership.context() {
        InteractionContext::HelpModal => {
            if keys.just_pressed(KeyCode::Escape) {
                intents.0.push(InputIntent::Key(KeyIntent::CloseHelp));
            }
        }
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
            if keys.just_pressed(KeyCode::Escape) {
                intents.0.push(InputIntent::Key(KeyIntent::OpenHelp));
            }
            let focused_widget_owns_activation = ownership.focused_widget_owns_activation();
            for binding in KEY_BINDINGS {
                if binding.key == KeyCode::Space && focused_widget_owns_activation {
                    continue;
                }
                if keys.just_pressed(binding.key) {
                    intents.0.push(InputIntent::Key(binding.intent));
                }
            }
        }
    }
    if ownership.blocks_gameplay() {
        motion.clear();
        wheel.clear();
        return;
    }
    if buttons.pressed(MouseButton::Right) && !capture.pointer_over_hud_surface {
        for event in motion.read() {
            intents.0.push(InputIntent::Orbit {
                delta_yaw: f64::from(event.delta.x),
                delta_pitch: f64::from(event.delta.y),
            });
        }
    } else {
        motion.clear();
    }
    if ownership.blocks_gameplay()
        || capture.pointer_over_scroll_surface
        || capture.pointer_over_hud_surface
    {
        wheel.clear();
    } else {
        for event in wheel.read() {
            intents.0.push(InputIntent::Dolly {
                delta: f64::from(event.y),
            });
        }
    }
}

fn collect_primary_drag(
    mut drag: On<Pointer<Drag>>,
    hud_surfaces: Query<Entity, With<HudSurface>>,
    parents: Query<&ChildOf>,
    current: InteractionInputs,
    interaction: Res<InteractionState>,
    mut state: ResMut<PrimaryDragState>,
    mut intents: ResMut<InputIntentQueue>,
) {
    let context = if interaction.context == InteractionContext::Gameplay {
        current.context()
    } else {
        interaction.context
    };
    if drag.button != PointerButton::Primary
        || context != InteractionContext::Gameplay
        || hud_surfaces
            .iter()
            .any(|root| is_descendant_of(drag.entity, root, &parents))
    {
        return;
    }
    if drag.distance.length() < PRIMARY_DRAG_THRESHOLD_PX {
        return;
    }
    let delta = if state.crossed_threshold {
        drag.delta
    } else {
        state.crossed_threshold = true;
        // Include movement accumulated before the threshold was crossed so
        // primary and secondary viewport drags have the same final orbit.
        drag.distance
    };
    intents.0.push(InputIntent::Orbit {
        delta_yaw: f64::from(delta.x),
        delta_pitch: f64::from(delta.y),
    });
    drag.propagate(false);
}

fn begin_primary_drag(press: On<Pointer<Press>>, mut state: ResMut<PrimaryDragState>) {
    if press.button == PointerButton::Primary {
        state.crossed_threshold = false;
    }
}

fn finish_primary_drag(drag: On<Pointer<DragEnd>>, mut state: ResMut<PrimaryDragState>) {
    if drag.button == PointerButton::Primary {
        state.crossed_threshold = false;
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
        InputIntent::Key(KeyIntent::TravelRegion(preset)) => {
            SimCommand::TravelToRegionPreset(preset)
        }
        InputIntent::Key(KeyIntent::StepRate(delta)) => SimCommand::StepRate(delta),
        InputIntent::Key(KeyIntent::Play) => SimCommand::Play,
        InputIntent::Key(KeyIntent::Pause) => SimCommand::Pause,
        InputIntent::Key(KeyIntent::TogglePlay) => SimCommand::TogglePlay,
        InputIntent::Key(KeyIntent::SetDayRate) => SimCommand::SetRate(day_rate()),
        InputIntent::Key(KeyIntent::ResetView) => SimCommand::ResetView,
        InputIntent::Key(KeyIntent::OpenHelp) => SimCommand::OpenHelp,
        InputIntent::Key(KeyIntent::CloseHelp) => SimCommand::CloseHelp,
        InputIntent::Key(KeyIntent::CloseSettings) => SimCommand::CloseSettings,
        InputIntent::Key(KeyIntent::CloseBrowse) => SimCommand::SetBrowseOpen(false),
        #[cfg(debug_assertions)]
        InputIntent::Key(KeyIntent::SimulateDeviceLoss) => SimCommand::SimulateDeviceLoss,
        #[cfg(debug_assertions)]
        InputIntent::Key(KeyIntent::ToggleDiagnosticsOverlay) => {
            SimCommand::ToggleDiagnosticsOverlay
        }
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

fn day_rate() -> RateIndex {
    // Index 4 is frozen as +1 day/s by ARCHITECTURE §4.2. Avoid exposing a
    // second time-ladder constant or panicking if that core contract changes.
    RateIndex::new(4).unwrap_or(RateIndex::REAL)
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
    use crate::control::consume_presentation_command;
    use crate::labels::{
        sync_label_emphasis_alpha, BodyLabel, BodyLabelText, BodyReticle, LabelEmphasisColor,
    };
    use crate::orbit_lines::rendered_orbit_brightness;
    use crate::search::consume_search_command;
    use crate::surface_textures::SaturnRing;
    use crate::ui_kit::test_layout;
    use crate::{
        load_catalog_text, propagate_catalog, replay_headless, visual_cue_recovery_needed,
        BodyVisual, CameraController, CommandRecording, HeadlessSimulation, LeftPanelTab,
        LoadedCatalog, OrbitLinesPlugin, ReplayStream, ScenePolishPlugin, SimulationClock,
        SimulationTickAdvance, ViewOptionsState, EMPHASIZED_ORBIT_BRIGHTNESS, ZOOM_IN_DOLLY_DELTA,
        ZOOM_OUT_DOLLY_DELTA,
    };
    use bevy::{
        camera::NormalizedRenderTarget,
        color::Alpha,
        ecs::entity::EntityHashMap,
        gizmos::GizmoAsset,
        input::{
            keyboard::{Key, KeyboardInput},
            ButtonState, InputPlugin,
        },
        input_focus::{FocusCause, FocusedInput, InputDispatchPlugin, InputFocusPlugin},
        picking::{
            backend::HitData,
            pointer::{Location, PointerId},
        },
        window::{PrimaryWindow, WindowRef},
    };
    use sim_core::time::{SimClock, StartMode};
    use std::{collections::HashSet, time::Duration};

    fn clear_focus_on_escape(
        mut input: On<FocusedInput<KeyboardInput>>,
        mut focus: ResMut<InputFocus>,
    ) {
        if input.input.state == ButtonState::Pressed && input.input.key_code == KeyCode::Escape {
            focus.clear();
            input.propagate(false);
        }
    }

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
    fn camera_rate_and_region_keys_map_to_the_reviewed_semantic_commands() {
        let command_for = |key| {
            let intent = KEY_BINDINGS
                .iter()
                .find_map(|binding| (binding.key == key).then_some(binding.intent))
                .unwrap();
            intent_to_command(InputIntent::Key(intent))
        };

        assert_eq!(
            command_for(KeyCode::ArrowLeft),
            command_for(KeyCode::BracketLeft)
        );
        assert_eq!(
            command_for(KeyCode::ArrowRight),
            command_for(KeyCode::BracketRight)
        );
        assert_eq!(
            command_for(KeyCode::ArrowDown),
            SimCommand::SetRate(day_rate())
        );
        assert_eq!(day_rate().label(), "1 DAY/S");
        assert_eq!(command_for(KeyCode::Home), SimCommand::ResetView);
        for (key, preset) in [
            (KeyCode::Digit1, RegionPreset::Inner),
            (KeyCode::Digit2, RegionPreset::Belt),
            (KeyCode::Digit3, RegionPreset::Outer),
            (KeyCode::Digit4, RegionPreset::Kuiper),
        ] {
            assert_eq!(command_for(key), SimCommand::TravelToRegionPreset(preset));
        }
    }

    #[test]
    fn escape_opens_help_only_from_gameplay_and_closes_it_as_the_modal_owner() {
        let mut open = interaction_test_app(false, false);
        open.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        open.update();
        assert_eq!(
            open.world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .collect::<Vec<_>>(),
            vec![SimCommand::OpenHelp]
        );

        let mut close = interaction_test_app(false, false);
        close
            .world_mut()
            .resource_mut::<PresentationState>()
            .open_help();
        {
            let mut keys = close.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            for key in [
                KeyCode::Escape,
                KeyCode::ArrowLeft,
                KeyCode::ArrowRight,
                KeyCode::ArrowDown,
                KeyCode::Home,
                KeyCode::Space,
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
            ] {
                keys.press(key);
            }
        }
        close.update();
        assert_eq!(
            close
                .world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .collect::<Vec<_>>(),
            vec![SimCommand::CloseHelp]
        );
        assert_eq!(
            close.world().resource::<InteractionState>().context(),
            InteractionContext::HelpModal
        );
    }

    #[test]
    fn primary_drag_threshold_matches_right_drag_and_hud_ownership() {
        fn location(window: Entity) -> Location {
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Entity(window).normalize(None).unwrap(),
                ),
                position: Vec2::new(100.0, 100.0),
            }
        }

        let mut primary = interaction_test_app(false, false);
        let window = primary.world_mut().spawn_empty().id();
        let target = primary.world_mut().spawn_empty().id();
        primary.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(window),
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::new(3.0, -2.0),
                delta: Vec2::new(3.0, -2.0),
            },
            target,
        ));
        assert!(!primary
            .world()
            .resource::<PrimaryDragState>()
            .blocks_click());
        primary.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(window),
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::new(6.0, -4.0),
                delta: Vec2::new(3.0, -2.0),
            },
            target,
        ));
        assert!(primary
            .world()
            .resource::<PrimaryDragState>()
            .blocks_click());
        primary.update();
        let primary_command = primary
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .next()
            .unwrap();

        let mut secondary = interaction_test_app(false, false);
        secondary
            .world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        secondary.world_mut().write_message(MouseMotion {
            delta: Vec2::new(6.0, -4.0),
        });
        secondary.update();
        let secondary_command = secondary
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .next()
            .unwrap();
        assert_eq!(primary_command, secondary_command);

        primary.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(window),
            DragEnd {
                button: PointerButton::Primary,
                distance: Vec2::new(6.0, -4.0),
            },
            target,
        ));
        assert!(!primary
            .world()
            .resource::<PrimaryDragState>()
            .blocks_click());

        let mut hud = interaction_test_app(false, false);
        let window = hud.world_mut().spawn_empty().id();
        let root = hud.world_mut().spawn(HudSurface).id();
        let target = hud.world_mut().spawn(ChildOf(root)).id();
        hud.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            location(window),
            Drag {
                button: PointerButton::Primary,
                distance: Vec2::new(20.0, 0.0),
                delta: Vec2::new(20.0, 0.0),
            },
            target,
        ));
        hud.update();
        assert_eq!(
            hud.world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .count(),
            0
        );
        assert!(!hud.world().resource::<PrimaryDragState>().blocks_click());
    }

    #[test]
    fn modal_and_text_contexts_block_gameplay_and_escape_has_one_owner() {
        for context in [
            InteractionContext::TextEdit,
            InteractionContext::BrowseModal,
            InteractionContext::SettingsModal,
            InteractionContext::HelpModal,
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

    fn interaction_test_app(browse_open: bool, settings_open: bool) -> App {
        let mut browse = BrowseUiState::default();
        if browse_open {
            consume_search_command(&SimCommand::SetBrowseOpen(true), &mut browse);
        }
        let mut presentation = PresentationState::default();
        if settings_open {
            presentation.open_settings();
        }
        let mut app = App::new();
        app.init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<MouseMotion>()
            .add_message::<MouseWheel>()
            .init_resource::<InputFocus>()
            .init_resource::<HoverMap>()
            .init_resource::<LayerState>()
            .insert_resource(browse)
            .insert_resource(presentation)
            .insert_resource(AppSettings::default())
            .init_resource::<SimCommandQueue>()
            .add_plugins(InputIntentPlugin);
        app
    }

    fn write_orbit_and_dolly_input(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(6.0, -4.0),
        });
        app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
    }

    #[test]
    fn stable_input_routing_state_does_not_advertise_false_changes() {
        #[derive(Resource, Debug, Default, PartialEq, Eq)]
        struct RoutingChanges {
            interaction: bool,
            pointer_capture: bool,
        }

        fn capture_changes(
            interaction: Res<InteractionState>,
            pointer_capture: Res<PointerCaptureState>,
            mut changes: ResMut<RoutingChanges>,
        ) {
            changes.interaction = interaction.is_changed();
            changes.pointer_capture = pointer_capture.is_changed();
        }

        let mut app = interaction_test_app(false, false);
        app.init_resource::<RoutingChanges>()
            .add_systems(PostUpdate, capture_changes);
        app.update();

        *app.world_mut().resource_mut::<RoutingChanges>() = RoutingChanges::default();
        app.update();

        assert_eq!(
            *app.world().resource::<RoutingChanges>(),
            RoutingChanges::default()
        );
    }

    #[test]
    fn focused_editable_text_blocks_every_gameplay_hotkey() {
        let mut app = interaction_test_app(false, false);
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
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::BracketLeft,
            KeyCode::BracketRight,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowDown,
            KeyCode::Home,
        ] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(6.0, -4.0),
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
        assert_eq!(
            app.world().resource::<InteractionState>().context(),
            InteractionContext::TextEdit
        );
    }

    #[test]
    fn focused_search_dropdown_result_keeps_text_edit_ownership() {
        let mut app = interaction_test_app(false, false);
        let dropdown = app.world_mut().spawn(SearchDropdownRoot).id();
        let result = app.world_mut().spawn((Button, ChildOf(dropdown))).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(result, FocusCause::Navigated);
        for key in [
            KeyCode::KeyS,
            KeyCode::KeyM,
            KeyCode::KeyI,
            KeyCode::KeyO,
            KeyCode::KeyR,
            KeyCode::KeyP,
            KeyCode::Space,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::BracketLeft,
            KeyCode::BracketRight,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowDown,
            KeyCode::Home,
        ] {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(6.0, -4.0),
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
        assert_eq!(
            app.world().resource::<InteractionState>().context(),
            InteractionContext::TextEdit
        );
    }

    #[test]
    fn browse_modal_blocks_hotkeys_and_escape_only_closes_browse() {
        let mut app = interaction_test_app(true, false);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            for key in [
                KeyCode::KeyS,
                KeyCode::Space,
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Escape,
            ] {
                keys.press(key);
            }
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
        let mut app = interaction_test_app(true, false);
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
    fn hovered_scroll_surface_owns_wheel_while_viewport_wheel_still_dollies() {
        let mut app = interaction_test_app(false, false);
        let camera = app.world_mut().spawn_empty().id();
        let surface = app.world_mut().spawn(UiScrollSurface).id();
        let hovered_child = app.world_mut().spawn(ChildOf(surface)).id();
        let mut hits = EntityHashMap::default();
        hits.insert(
            hovered_child,
            HitData {
                camera,
                depth: 0.0,
                position: None,
                normal: None,
                extra: None,
            },
        );
        app.world_mut()
            .resource_mut::<HoverMap>()
            .insert(PointerId::Mouse, hits);
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

        app.world_mut().resource_mut::<HoverMap>().clear();
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
                .collect::<Vec<_>>(),
            vec![SimCommand::Dolly { delta: 2.0 }]
        );
    }

    #[test]
    fn hovered_hud_surfaces_own_drag_and_wheel_while_the_viewport_remains_gameplay() {
        let mut app = interaction_test_app(false, false);
        let camera = app.world_mut().spawn_empty().id();

        for surface_name in [
            "Top bar",
            "Left panel",
            "Right rail",
            "Layers quick panel",
            "Settings screen",
        ] {
            let surface = app
                .world_mut()
                .spawn((Name::new(surface_name), HudSurface))
                .id();
            let hovered_child = app.world_mut().spawn(ChildOf(surface)).id();
            let mut hits = EntityHashMap::default();
            hits.insert(
                hovered_child,
                HitData {
                    camera,
                    depth: 0.0,
                    position: None,
                    normal: None,
                    extra: None,
                },
            );
            app.world_mut()
                .resource_mut::<HoverMap>()
                .insert(PointerId::Mouse, hits);
            write_orbit_and_dolly_input(&mut app);

            app.update();

            assert_eq!(
                app.world_mut()
                    .resource_mut::<SimCommandQueue>()
                    .drain()
                    .count(),
                0,
                "{surface_name} leaked raw camera input"
            );
        }

        app.world_mut().resource_mut::<HoverMap>().clear();
        write_orbit_and_dolly_input(&mut app);
        app.update();

        assert_eq!(
            app.world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .collect::<Vec<_>>(),
            vec![
                SimCommand::Orbit {
                    delta_yaw: 6.0,
                    delta_pitch: -4.0,
                },
                SimCommand::Dolly { delta: 2.0 },
            ]
        );
    }

    #[test]
    fn keyboard_focus_scrolls_the_target_into_its_registered_surface() {
        let mut app = test_layout::app(800, 600, 2.0);
        app.add_systems(Update, ensure_focused_control_visible)
            .add_observer(scroll_registered_surface_into_view);
        let surface = app
            .world_mut()
            .spawn((
                UiScrollSurface,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: px(200),
                    height: px(100),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .id();
        let mut last = Entity::PLACEHOLDER;
        for index in 0..5 {
            last = app
                .world_mut()
                .spawn((
                    TabIndex(index),
                    Node {
                        width: percent(100),
                        min_height: px(50),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ChildOf(surface),
                ))
                .id();
        }
        test_layout::settle(&mut app);
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(last, FocusCause::Navigated);
        test_layout::settle(&mut app);

        let position = app.world().entity(surface).get::<ScrollPosition>().unwrap();
        assert!(position.y > 0.0);
        let surface_rect = layout_node_rect(app.world(), surface);
        let target_rect = layout_node_rect(app.world(), last);
        assert!(
            target_rect.min.y >= surface_rect.min.y - 1.0
                && target_rect.max.y <= surface_rect.max.y + 1.0
        );
    }

    fn layout_node_rect(world: &World, entity: Entity) -> Rect {
        let node = world.get::<ComputedNode>(entity).unwrap();
        let center = world
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .affine()
            .translation;
        Rect::from_center_size(center, node.size())
    }

    #[test]
    fn settings_modal_blocks_all_gameplay_input_and_escape_only_closes_settings() {
        let mut app = interaction_test_app(false, true);
        {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            for key in [
                KeyCode::KeyM,
                KeyCode::Space,
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
                KeyCode::Escape,
            ] {
                keys.press(key);
            }
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(4.0, -2.0),
        });
        app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });

        app.update();

        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::CloseSettings]);
    }

    #[test]
    fn focused_button_owns_space_without_a_second_global_toggle() {
        let mut app = interaction_test_app(false, false);
        let button = app.world_mut().spawn(bevy::ui_widgets::Button).id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<SimCommandQueue>()
            .push(SimCommand::SetBrowseOpen(true));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);

        app.update();

        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::SetBrowseOpen(true)]);
    }

    #[test]
    fn defensive_context_priority_matches_escape_acceptance_order() {
        assert_eq!(
            resolve_interaction_context(true, true, true, true),
            InteractionContext::TextEdit
        );
        assert_eq!(
            resolve_interaction_context(false, true, true, true),
            InteractionContext::BrowseModal
        );
        assert_eq!(
            resolve_interaction_context(false, false, true, true),
            InteractionContext::SettingsModal
        );
        assert_eq!(
            resolve_interaction_context(false, false, false, true),
            InteractionContext::HelpModal
        );
        assert_eq!(
            resolve_interaction_context(false, false, false, false),
            InteractionContext::Gameplay
        );
    }

    #[test]
    fn malformed_overlapping_modals_escape_closes_only_browse() {
        let mut app = interaction_test_app(true, true);
        app.world_mut()
            .resource_mut::<PresentationState>()
            .open_help();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);

        app.update();

        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::SetBrowseOpen(false)]);
    }

    #[test]
    fn text_edit_claim_survives_preupdate_focus_clear_and_blocks_same_frame_gameplay() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            InputPlugin,
            InputFocusPlugin,
            InputDispatchPlugin,
        ))
        .init_resource::<HoverMap>()
        .init_resource::<BrowseUiState>()
        .init_resource::<PresentationState>()
        .init_resource::<AppSettings>()
        .init_resource::<SimCommandQueue>()
        .add_plugins(InputIntentPlugin);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let editable = app
            .world_mut()
            .spawn(EditableText::new(""))
            .observe(clear_focus_on_escape)
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(editable, FocusCause::Navigated);
        app.update();
        for (key_code, logical_key) in [
            (KeyCode::Escape, Key::Escape),
            (KeyCode::KeyS, Key::Character("s".into())),
        ] {
            app.world_mut().write_message(KeyboardInput {
                key_code,
                logical_key,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window,
            });
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(3.0, 2.0),
        });
        app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            window,
            phase: bevy::input::touch::TouchPhase::Moved,
        });

        app.update();

        assert_eq!(app.world().resource::<InputFocus>().get(), None);
        assert_eq!(
            app.world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .count(),
            0
        );
    }

    #[test]
    fn modal_focus_reconciliation_follows_the_canonical_active_modal() {
        let mut app = interaction_test_app(true, false);
        let browse_root = app.world_mut().spawn(BrowseMenuRoot).id();
        let browse_button = app
            .world_mut()
            .spawn((TabIndex(10), ChildOf(browse_root)))
            .id();
        let settings_root = app.world_mut().spawn(SettingsScreenRoot).id();
        let settings_button = app
            .world_mut()
            .spawn((TabIndex(10), ChildOf(settings_root)))
            .id();
        let help_root = app.world_mut().spawn(HelpModalRoot).id();
        let help_button = app
            .world_mut()
            .spawn((TabIndex(10), ChildOf(help_root)))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(settings_button, FocusCause::Navigated);

        app.update();

        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(browse_button)
        );

        consume_search_command(
            &SimCommand::SetBrowseOpen(false),
            &mut app.world_mut().resource_mut::<BrowseUiState>(),
        );
        app.world_mut()
            .resource_mut::<PresentationState>()
            .open_settings();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(browse_button, FocusCause::Navigated);

        app.update();

        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(settings_button)
        );

        {
            let mut presentation = app.world_mut().resource_mut::<PresentationState>();
            presentation.close_settings();
            presentation.open_help();
        }
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(settings_button, FocusCause::Navigated);

        app.update();

        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(help_button)
        );
    }

    #[test]
    fn ui_off_restore_focus_outranks_hidden_modals_until_ui_returns() {
        let mut app = interaction_test_app(true, true);
        let restore = app
            .world_mut()
            .spawn((UiRestoreAffordance, TabIndex(0)))
            .id();
        let browse_root = app.world_mut().spawn(BrowseMenuRoot).id();
        let browse_button = app
            .world_mut()
            .spawn((TabIndex(0), ChildOf(browse_root)))
            .id();
        let settings_root = app.world_mut().spawn(SettingsScreenRoot).id();
        let settings_button = app
            .world_mut()
            .spawn((TabIndex(0), ChildOf(settings_root)))
            .id();
        app.world_mut()
            .resource_mut::<LayerState>()
            .set_visible(LayerId::UserInterface, false);
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(settings_button, FocusCause::Navigated);

        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));

        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(browse_button, FocusCause::Navigated);
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(restore));

        app.world_mut()
            .resource_mut::<LayerState>()
            .set_visible(LayerId::UserInterface, true);
        app.update();
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(browse_button),
            "Browse remains the canonical higher-priority modal once UI returns"
        );
    }

    #[test]
    fn composed_real_catalog_stabilization_lifecycle_crosses_every_interaction_boundary() {
        fn recorded_step(
            simulation: &mut HeadlessSimulation,
            recording: &mut CommandRecording,
            wall_now_t: &mut f64,
            wall_delta_s: f64,
            commands: &[SimCommand],
        ) {
            *wall_now_t += wall_delta_s;
            simulation
                .step_with_wall_time(wall_delta_s, *wall_now_t, commands, Some(recording))
                .unwrap();
        }

        // Text ownership, modal routing, and pointer capture are exercised
        // through one real input-plugin lifecycle.
        let mut interaction_app = interaction_test_app(false, false);
        let editable = interaction_app
            .world_mut()
            .spawn(EditableText::new("io"))
            .id();
        interaction_app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(editable, FocusCause::Navigated);
        {
            let mut keys = interaction_app
                .world_mut()
                .resource_mut::<ButtonInput<KeyCode>>();
            keys.press(KeyCode::KeyS);
            keys.press(KeyCode::Space);
            keys.press(KeyCode::Digit1);
        }
        interaction_app
            .world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        interaction_app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(8.0, -3.0),
        });
        interaction_app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        interaction_app.update();
        assert_eq!(
            interaction_app
                .world()
                .resource::<InteractionState>()
                .context(),
            InteractionContext::TextEdit
        );
        assert_eq!(
            interaction_app.world().resource::<InputFocus>().get(),
            Some(editable)
        );
        assert_eq!(
            interaction_app
                .world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .count(),
            0,
            "focused text must suppress gameplay keyboard, drag, and wheel input"
        );
        interaction_app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        interaction_app
            .world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .reset_all();
        interaction_app
            .world_mut()
            .resource_mut::<InputFocus>()
            .clear();

        // Browse retains priority over Settings, then hands ownership back in
        // the documented order without exposing gameplay between modals. Both
        // Escape actions pass through the raw-input-to-command boundary.
        consume_search_command(
            &SimCommand::SetBrowseOpen(true),
            &mut interaction_app.world_mut().resource_mut::<BrowseUiState>(),
        );
        interaction_app
            .world_mut()
            .resource_mut::<PresentationState>()
            .open_settings();
        interaction_app.update();
        assert_eq!(
            interaction_app
                .world()
                .resource::<InteractionState>()
                .context(),
            InteractionContext::BrowseModal
        );
        interaction_app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        interaction_app.update();
        let browse_commands = interaction_app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            browse_commands,
            vec![SimCommand::SetBrowseOpen(false)],
            "Browse must be the sole owner of Escape while both modals exist"
        );
        let close_browse = &browse_commands[0];
        consume_search_command(
            close_browse,
            &mut interaction_app.world_mut().resource_mut::<BrowseUiState>(),
        );
        interaction_app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        interaction_app.update();
        assert_eq!(
            interaction_app
                .world()
                .resource::<InteractionState>()
                .context(),
            InteractionContext::SettingsModal
        );
        interaction_app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        interaction_app.update();
        let settings_commands = interaction_app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            settings_commands,
            vec![SimCommand::CloseSettings],
            "Settings must become the sole Escape owner after Browse closes"
        );
        let close_settings = &settings_commands[0];
        interaction_app
            .world_mut()
            .resource_scope(|world, mut layers: Mut<LayerState>| {
                let mut presentation = world.resource_mut::<PresentationState>();
                consume_presentation_command(close_settings, &mut layers, &mut presentation);
            });
        interaction_app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        interaction_app.update();
        assert_eq!(
            interaction_app
                .world()
                .resource::<InteractionState>()
                .context(),
            InteractionContext::Gameplay
        );

        let hover_camera = interaction_app.world_mut().spawn_empty().id();
        let hover_surface = interaction_app.world_mut().spawn(UiScrollSurface).id();
        let hovered_child = interaction_app
            .world_mut()
            .spawn(ChildOf(hover_surface))
            .id();
        let mut hits = EntityHashMap::default();
        hits.insert(
            hovered_child,
            HitData {
                camera: hover_camera,
                depth: 0.0,
                position: None,
                normal: None,
                extra: None,
            },
        );
        interaction_app
            .world_mut()
            .resource_mut::<HoverMap>()
            .insert(PointerId::Mouse, hits);
        interaction_app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        interaction_app.update();
        assert_eq!(
            interaction_app
                .world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .count(),
            0,
            "a hovered scroll surface must own the wheel instead of Dolly"
        );
        interaction_app
            .world_mut()
            .resource_mut::<HoverMap>()
            .clear();
        interaction_app.world_mut().write_message(MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Line,
            x: 0.0,
            y: 2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        interaction_app.update();
        assert_eq!(
            interaction_app
                .world_mut()
                .resource_mut::<SimCommandQueue>()
                .drain()
                .collect::<Vec<_>>(),
            vec![SimCommand::Dolly { delta: 2.0 }],
            "the same wheel event must return to camera Dolly over the viewport"
        );

        // Keyboard focus moves a real registered scroll surface just enough
        // to keep its final action reachable.
        let mut scroll_app = test_layout::app(800, 600, 2.0);
        scroll_app
            .add_systems(Update, ensure_focused_control_visible)
            .add_observer(scroll_registered_surface_into_view);
        let surface = scroll_app
            .world_mut()
            .spawn((
                UiScrollSurface,
                Node {
                    width: px(200),
                    height: px(100),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .id();
        let mut last = Entity::PLACEHOLDER;
        for index in 0..5 {
            last = scroll_app
                .world_mut()
                .spawn((
                    TabIndex(index),
                    Node {
                        width: percent(100),
                        min_height: px(50),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ChildOf(surface),
                ))
                .id();
        }
        test_layout::settle(&mut scroll_app);
        scroll_app
            .world_mut()
            .resource_mut::<InputFocus>()
            .set(last, FocusCause::Navigated);
        test_layout::settle(&mut scroll_app);
        assert_eq!(
            scroll_app.world().resource::<InputFocus>().get(),
            Some(last)
        );
        assert!(
            scroll_app
                .world()
                .entity(surface)
                .get::<ScrollPosition>()
                .unwrap()
                .y
                > 0.0
        );

        let catalog = load_catalog_text(include_str!("../../../assets/catalog.ron")).unwrap();
        let jupiter = catalog
            .bodies
            .iter()
            .position(|body| body.id == "jupiter")
            .unwrap();
        let io = catalog
            .bodies
            .iter()
            .position(|body| body.id == "io")
            .unwrap();
        let mut simulation = HeadlessSimulation::new(&catalog).unwrap();
        let mut recording = CommandRecording::default();
        let mut wall_now_t = simulation.clock().t();
        let frame_delta_s = 1.0 / 60.0;

        recorded_step(
            &mut simulation,
            &mut recording,
            &mut wall_now_t,
            frame_delta_s,
            &[
                SimCommand::TravelToBody("jupiter".into()),
                SimCommand::SetLeftPanelTab(LeftPanelTab::Collection),
            ],
        );
        assert_eq!(simulation.camera().selected_body_index(), jupiter);
        assert_eq!(
            simulation.navigation_label(),
            "Solar System › Jupiter › Moons"
        );
        recorded_step(
            &mut simulation,
            &mut recording,
            &mut wall_now_t,
            1.0 / 90.0,
            &[SimCommand::TravelToBody("io".into())],
        );
        assert_eq!(simulation.camera().selected_body_index(), io);
        assert_eq!(simulation.navigation_label(), "Solar System › Jupiter › Io");

        recorded_step(
            &mut simulation,
            &mut recording,
            &mut wall_now_t,
            1.0 / 48.0,
            &[
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Orbits,
                    visible: false,
                },
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Labels,
                    visible: false,
                },
                SimCommand::SetLayerVisibility {
                    layer: LayerId::Icons,
                    visible: false,
                },
            ],
        );
        assert!(visual_cue_recovery_needed(*simulation.layer_state()));
        recorded_step(
            &mut simulation,
            &mut recording,
            &mut wall_now_t,
            frame_delta_s,
            &[
                SimCommand::ApplySettings(Box::default()),
                SimCommand::RestorePresentationDefaults,
            ],
        );
        assert_eq!(simulation.app_settings(), &AppSettings::default());
        assert_eq!(simulation.layer_state(), &LayerState::default());
        assert!(!visual_cue_recovery_needed(*simulation.layer_state()));
        assert!(simulation.settings_save_requested());

        let before_high_rate = simulation.clock().t();
        recorded_step(
            &mut simulation,
            &mut recording,
            &mut wall_now_t,
            frame_delta_s,
            &[SimCommand::SetRate(RateIndex::MAX)],
        );
        let high_rate_advance = (simulation.clock().t() - before_high_rate).abs();
        assert!(high_rate_advance > 0.0);

        // The same body-indexed emphasis state drives Saturn's sphere/ring,
        // text, and orbit plus Io's architecture-valid reticle.
        let loaded = LoadedCatalog::new(catalog.clone());
        let saturn = loaded.index_of("saturn").unwrap();
        let io = loaded.index_of("io").unwrap();
        let sun = loaded.index_of("sun").unwrap();
        let saturn_color = loaded.catalog.bodies[saturn].color_srgb;
        let io_color = loaded.catalog.bodies[io].color_srgb;
        let render_t = simulation.clock().t();
        let render_states = propagate_catalog(&loaded.catalog, render_t).unwrap();
        let render_camera =
            CameraController::new(sun, render_states.0[sun].position_km, 3_000_000.0);
        let render_clock = SimClock::new(
            StartMode::FixedEpoch {
                jd_tdb: sim_core::time::jd_tdb_from_t(render_t),
            },
            render_t,
        );
        let mut render_app = App::new();
        render_app
            .init_resource::<Time<Real>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<GizmoAsset>>()
            .insert_resource(loaded)
            .insert_resource(render_states)
            .insert_resource(render_camera)
            .insert_resource(SimulationClock(render_clock))
            .insert_resource(LayerState::default())
            .insert_resource(ViewOptionsState::default())
            .add_plugins((ScenePolishPlugin, OrbitLinesPlugin))
            .add_systems(PostUpdate, sync_label_emphasis_alpha);
        let (sphere_material, ring_material) = {
            let mut materials = render_app
                .world_mut()
                .resource_mut::<Assets<StandardMaterial>>();
            (
                materials.add(StandardMaterial {
                    base_color: Color::srgb_u8(saturn_color.0, saturn_color.1, saturn_color.2),
                    ..default()
                }),
                materials.add(StandardMaterial {
                    base_color: Color::srgba(0.92, 0.86, 0.72, 0.9),
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                }),
            )
        };
        render_app.world_mut().spawn((
            BodyVisual { index: saturn },
            MeshMaterial3d(sphere_material.clone()),
        ));
        render_app.world_mut().spawn((
            SaturnRing { body_index: saturn },
            MeshMaterial3d(ring_material.clone()),
        ));
        let saturn_label = render_app
            .world_mut()
            .spawn((BodyLabel { index: saturn }, Node::default()))
            .id();
        let saturn_text_base = Color::srgb_u8(saturn_color.0, saturn_color.1, saturn_color.2);
        let saturn_text = render_app
            .world_mut()
            .spawn((
                BodyLabelText,
                LabelEmphasisColor {
                    body_index: saturn,
                    base_color: saturn_text_base,
                },
                TextColor(saturn_text_base),
                ChildOf(saturn_label),
            ))
            .id();
        let io_label = render_app
            .world_mut()
            .spawn((BodyLabel { index: io }, Node::default()))
            .id();
        let io_reticle_base = Color::srgb_u8(io_color.0, io_color.1, io_color.2);
        let io_reticle = render_app
            .world_mut()
            .spawn((
                BodyReticle,
                LabelEmphasisColor {
                    body_index: io,
                    base_color: io_reticle_base,
                },
                BorderColor::all(io_reticle_base),
                ChildOf(io_label),
            ))
            .id();
        render_app.update();
        for _ in 0..15 {
            *render_app
                .world_mut()
                .resource_mut::<SimulationTickAdvance>() =
                SimulationTickAdvance::between(0.0, high_rate_advance);
            render_app
                .world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_secs_f64(frame_delta_s));
            render_app.update();
        }
        {
            let materials = render_app.world().resource::<Assets<StandardMaterial>>();
            assert_eq!(
                materials.get(&sphere_material).unwrap().base_color.alpha(),
                0.0
            );
            assert_eq!(
                materials.get(&ring_material).unwrap().base_color.alpha(),
                0.0
            );
        }
        assert_eq!(
            render_app
                .world()
                .entity(saturn_text)
                .get::<TextColor>()
                .unwrap()
                .0
                .alpha(),
            0.0
        );
        assert_eq!(
            render_app
                .world()
                .entity(io_reticle)
                .get::<BorderColor>()
                .unwrap()
                .top
                .alpha(),
            0.0
        );
        assert_eq!(
            rendered_orbit_brightness(render_app.world_mut(), saturn),
            Some(EMPHASIZED_ORBIT_BRIGHTNESS)
        );
        assert_eq!(
            rendered_orbit_brightness(render_app.world_mut(), io),
            Some(EMPHASIZED_ORBIT_BRIGHTNESS)
        );

        for frame in 0..240 {
            let wall_delta_s = match frame % 3 {
                0 => 1.0 / 48.0,
                1 => 1.0 / 60.0,
                _ => 1.0 / 90.0,
            };
            let commands = if frame == 0 {
                vec![SimCommand::SnapToLive]
            } else {
                Vec::new()
            };
            recorded_step(
                &mut simulation,
                &mut recording,
                &mut wall_now_t,
                wall_delta_s,
                &commands,
            );
        }
        assert!(simulation.clock().is_live(wall_now_t));
        let replay = ReplayStream::from_text(&recording.stream().to_text()).unwrap();
        assert_eq!(&replay, recording.stream());
        let replayed =
            replay_headless(&catalog, &replay, simulation.frame(), frame_delta_s).unwrap();
        assert_eq!(replayed.state_hash(), simulation.state_hash());
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
