//! WP8 Eyes-style time bar — Rev C §§4.2, 7, and 9.5–9.6.
//!
//! Presentation binds directly to WP1's clock API. Edits and controls enqueue
//! `SimCommand`s, while toasts consume transition-only `TickReport`s; neither
//! path reimplements clock levels or time arithmetic.

use crate::control::{SimCommand, SimCommandQueue};
use crate::layers::HudSurface;
use crate::ui_kit::{toast, UiColorToken, UiTheme, WidgetSpec, WidgetVisualState};
use crate::{
    wall_now_t, ClockTickReport, OrbitEmphasisOnset, SimulationClock, SimulationSet,
    INTER_FONT_ASSET,
};
use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        tab_navigation::{TabIndex, TabNavigationPlugin},
        FocusedInput, InputFocus,
    },
    prelude::*,
    text::{EditableText, FontSourceTemplate, LetterSpacing, LineBreak, TextEdit, TextLayout},
    ui_widgets::{
        Activate, Slider, SliderDragState, SliderPrecision, SliderRange, SliderStep, SliderThumb,
        SliderValue, TrackClick, ValueChange,
    },
};
use sim_core::time::{
    datetime_from_t, format_date_eyes, parse_date, parse_time, t_from_datetime, DateTime,
    RangeEdge, RateIndex, SimClock, TickReport,
};

pub const TIME_BAR_HEIGHT_PX: f32 = 84.0;
const TOAST_LIFETIME_S: f32 = 4.0;
const SLIDER_LIMIT: f32 = 12.0;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
pub struct TimeBarRoot;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct TimeRateLabel;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct TimeRateSlider;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct TimeSliderThumb;

#[derive(Component, Debug, Clone, Copy, Default)]
struct TimeSliderDetent;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct PlayPauseButton;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct PlayPauseGlyph;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct LiveChip;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct LiveDot;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct LiveText;

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct TimeToastStack;

#[derive(Component, Debug, Clone, Copy)]
struct TimeToast {
    remaining_s: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeEditField {
    #[default]
    Date,
    Clock,
}

#[derive(Component, Debug, Clone, Copy, Default, FromTemplate)]
struct TimeEditFieldComponent {
    field: TimeEditField,
}

#[derive(Resource, Debug, Default)]
struct TimeEditFocus {
    active: Option<Entity>,
    original_t_s: Option<f64>,
    cancelled: Option<Entity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeEditOutcome {
    pub accepted: bool,
    pub t_s: f64,
    pub display: String,
}

/// Applies one strict date or clock edit without mutating a `SimClock`.
/// Callers enqueue `SetTime` only when `accepted` is true.
pub fn commit_time_edit(current_t_s: f64, field: TimeEditField, input: &str) -> TimeEditOutcome {
    let mut datetime = datetime_from_t(current_t_s);
    let parsed = match field {
        TimeEditField::Date => parse_date(input).map(|(year, month, day)| {
            datetime.year = year;
            datetime.month = month;
            datetime.day = day;
        }),
        TimeEditField::Clock => parse_time(input).map(|(hour, minute, second)| {
            datetime.hour = hour;
            datetime.minute = minute;
            datetime.second = second;
        }),
    };
    let target = parsed.and_then(|()| t_from_datetime(&datetime));
    match target {
        Ok(t_s) => TimeEditOutcome {
            accepted: true,
            t_s,
            display: display_value(field, t_s),
        },
        Err(_) => TimeEditOutcome {
            accepted: false,
            t_s: current_t_s,
            display: display_value(field, current_t_s),
        },
    }
}

/// Converts the UI's integral −12…12 track into WP1's signed ladder. Zero is
/// the separate paused position and therefore has no `RateIndex` value.
pub fn rate_for_slider_value(value: f32) -> Option<RateIndex> {
    let detent = value.clamp(-SLIDER_LIMIT, SLIDER_LIMIT).round();
    if detent == 0.0 {
        None
    } else {
        Some(RateIndex::from_slider_pos(detent / SLIDER_LIMIT))
    }
}

pub fn slider_value_for_rate(rate: RateIndex) -> f32 {
    rate.slider_pos() * SLIDER_LIMIT
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeToastKind {
    RangeMinimum,
    RangeMaximum,
    Extrapolation,
    SnappedLive,
    OrbitEmphasis,
}

impl TimeToastKind {
    fn text(self) -> &'static str {
        match self {
            Self::RangeMinimum => "TIME RANGE · CLAMPED AT 1800",
            Self::RangeMaximum => "TIME RANGE · CLAMPED AT 2300",
            Self::Extrapolation => "POSITIONS ARE EXTRAPOLATED OUTSIDE 1800–2050",
            Self::SnappedLive => "LIVE TIME RESTORED",
            Self::OrbitEmphasis => "Inner orbits shown as paths at this speed",
        }
    }
}

/// Maps transition payloads to notices. The absence of a payload produces no
/// toast; clock levels are deliberately not inspected here.
pub fn toasts_for_tick_report(report: TickReport) -> Vec<TimeToastKind> {
    let mut notices = Vec::with_capacity(3);
    if let Some(edge) = report.clamped {
        notices.push(match edge {
            RangeEdge::AtMin => TimeToastKind::RangeMinimum,
            RangeEdge::AtMax => TimeToastKind::RangeMaximum,
        });
    }
    if report.extrapolation_changed == Some(true) {
        notices.push(TimeToastKind::Extrapolation);
    }
    if report.snapped_live {
        notices.push(TimeToastKind::SnappedLive);
    }
    notices
}

pub fn live_chip_active(clock: &SimClock, wall_now_t_s: f64) -> bool {
    clock.is_live(wall_now_t_s)
}

pub struct TimeBarPlugin;

impl Plugin for TimeBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TabNavigationPlugin)
            .init_resource::<TimeEditFocus>()
            .add_systems(Startup, spawn_time_bar)
            .add_systems(Update, update_time_edits.in_set(SimulationSet::Input))
            .add_systems(
                Update,
                (
                    spawn_slider_detents,
                    sync_time_playback_controls,
                    sync_live_chip,
                    update_slider_thumb,
                    consume_tick_reports,
                    consume_orbit_emphasis_onsets,
                    expire_time_toasts,
                )
                    .chain()
                    .in_set(SimulationSet::Render),
            );
    }
}

fn time_bar_scene(theme: UiTheme, clock: &SimClock) -> impl Scene {
    let datetime = clock.datetime();
    let rate_label = if clock.is_playing() {
        clock.rate().label()
    } else {
        "PAUSED".to_string()
    };
    let tracking = theme.type_scale.uppercase_tracking_px;
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            bottom: px(0),
            width: percent(100),
            height: px(TIME_BAR_HEIGHT_PX),
            padding: UiRect::horizontal(px(theme.spacing.lg_px)),
            border: UiRect::top(px(theme.spacing.hairline_px)),
            align_items: AlignItems::Center,
            column_gap: px(theme.spacing.md_px),
        }
        TimeBarRoot
        HudSurface
        AccessibleLabel("Simulation time bar")
        BackgroundColor({theme.colors.top_bar.color()})
        BorderColor::all(theme.colors.separator.color())
        GlobalZIndex(95)
        Children [
            edit_field(theme, TimeEditField::Date, format_date_eyes(&datetime), "Simulation date"),
            edit_field(theme, TimeEditField::Clock, format_clock(datetime), "Simulation clock"),
            (
                Node {
                    width: px(theme.spacing.hairline_px),
                    height: px(32),
                }
                BackgroundColor({theme.colors.separator.color()})
            ),
            play_pause_button(theme, clock.is_playing()),
            (
                Node {
                    flex_grow: 1.0,
                    min_width: px(280),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.spacing.sm_px),
                }
                Children [
                    (
                        Text(rate_label)
                        TimeRateLabel
                        TextFont {
                            font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                            font_size: px(theme.type_scale.caption_px),
                        }
                        TextColor({theme.colors.text_muted.color()})
                        template_value(LetterSpacing::Px(tracking))
                    ),
                    rate_slider(theme, clock),
                ]
            ),
            live_chip(theme),
        ]
    }
}

fn edit_field(
    theme: UiTheme,
    field: TimeEditField,
    value: String,
    accessible_label: &'static str,
) -> impl Scene {
    let width = match field {
        TimeEditField::Date => 154.0,
        TimeEditField::Clock => 92.0,
    };
    bsn! {
        Node {
            width: px(width),
            height: px(38),
            padding: UiRect::horizontal(px(theme.spacing.sm_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
        }
        BackgroundColor({theme.colors.background.color()})
        BorderColor::all(theme.colors.separator.color())
        Children [(
            template_value(EditableText::new(value))
            TimeEditFieldComponent { field }
            AccessibleLabel(accessible_label)
            TabIndex(0)
            Node {
                width: percent(100),
            }
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.body_px),
            }
            TextColor({theme.colors.text_primary.color()})
            TextLayout {
                linebreak: LineBreak::NoWrap,
            }
            on(finish_time_edit)
        )]
    }
}

fn play_pause_button(theme: UiTheme, playing: bool) -> impl Scene {
    let background = if playing {
        with_alpha(theme.colors.accent, 40)
    } else {
        theme.colors.panel.color()
    };
    let glyph = if playing { "Ⅱ" } else { "▶" };
    bsn! {
        Node {
            width: px(42),
            height: px(38),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        bevy::ui_widgets::Button
        PlayPauseButton
        AccessibleLabel("Pause or resume simulation time")
        TabIndex(0)
        BackgroundColor(background)
        BorderColor::all(theme.colors.accent.color())
        on(toggle_play_pause)
        Children [(
            Text(glyph)
            PlayPauseGlyph
            TextFont {
                font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                font_size: px(theme.type_scale.title_px),
            }
            TextColor({theme.colors.text_primary.color()})
        )]
    }
}

fn rate_slider(theme: UiTheme, clock: &SimClock) -> impl Scene {
    let value = if clock.is_playing() {
        slider_value_for_rate(clock.rate())
    } else {
        0.0
    };
    bsn! {
        Node {
            width: percent(100),
            height: px(18),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
        }
        Slider {
            track_click: TrackClick::Snap,
        }
        TimeRateSlider
        template_value(SliderValue(value))
        template_value(SliderRange::new(-SLIDER_LIMIT, SLIDER_LIMIT))
        SliderStep(1.0)
        SliderPrecision(0)
        AccessibleLabel("Simulation rate: 24 signed detents and paused center")
        TabIndex(0)
        on(change_rate_slider)
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(4),
                    border_radius: BorderRadius::MAX,
                }
                BackgroundColor({theme.colors.separator.color()})
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(14),
                    top: px(0),
                    bottom: px(0),
                }
                Children [(
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50),
                        width: px(14),
                        height: px(14),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::MAX,
                    }
                    TimeSliderThumb
                    SliderThumb
                    BackgroundColor({theme.colors.accent.color()})
                    BorderColor::all(theme.colors.background.color())
                )]
            ),
        ]
    }
}

fn live_chip(theme: UiTheme) -> impl Scene {
    bsn! {
        Node {
            width: px(82),
            height: px(34),
            padding: UiRect::horizontal(px(theme.spacing.sm_px)),
            border: UiRect::all(px(theme.spacing.hairline_px)),
            border_radius: BorderRadius::MAX,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: px(theme.spacing.sm_px),
        }
        bevy::ui_widgets::Button
        LiveChip
        AccessibleLabel("Snap simulation to LIVE time")
        TabIndex(0)
        BackgroundColor({theme.colors.background.color()})
        BorderColor::all(theme.colors.separator.color())
        on(snap_to_live)
        Children [
            (
                Node {
                    width: px(7),
                    height: px(7),
                    border_radius: BorderRadius::MAX,
                }
                LiveDot
                BackgroundColor({theme.colors.text_disabled.color()})
            ),
            (
                Text("LIVE")
                LiveText
                TextFont {
                    font: FontSourceTemplate::Handle(INTER_FONT_ASSET),
                    font_size: px(theme.type_scale.caption_px),
                }
                TextColor({theme.colors.text_disabled.color()})
            ),
        ]
    }
}

fn toast_stack(theme: UiTheme) -> impl Scene {
    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(theme.spacing.lg_px),
            bottom: px(TIME_BAR_HEIGHT_PX + theme.spacing.lg_px),
            width: px(390),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.spacing.sm_px),
        }
        TimeToastStack
        HudSurface
        AccessibleLabel("Simulation notices")
        GlobalZIndex(105)
    }
}

fn spawn_time_bar(mut commands: Commands, theme: Res<UiTheme>, clock: Res<SimulationClock>) {
    commands.spawn_scene(time_bar_scene(*theme, &clock.0));
    commands.spawn_scene(toast_stack(*theme));
}

fn finish_time_edit(
    mut input: On<FocusedInput<KeyboardInput>>,
    mut focus: ResMut<InputFocus>,
    mut state: ResMut<TimeEditFocus>,
) {
    if input.input.state != ButtonState::Pressed {
        return;
    }
    match input.input.key_code {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            state.cancelled = None;
            focus.clear();
            input.propagate(false);
        }
        KeyCode::Escape => {
            state.cancelled = Some(input.focused_entity);
            focus.clear();
            input.propagate(false);
        }
        _ => {}
    }
}

fn toggle_play_pause(_activate: On<Activate>, mut commands: ResMut<SimCommandQueue>) {
    commands.push(SimCommand::TogglePlay);
}

fn snap_to_live(_activate: On<Activate>, mut commands: ResMut<SimCommandQueue>) {
    commands.push(SimCommand::SnapToLive);
}

fn change_rate_slider(
    change: On<ValueChange<f32>>,
    mut commands: Commands,
    mut sim_commands: ResMut<SimCommandQueue>,
) {
    let value = change.value.clamp(-SLIDER_LIMIT, SLIDER_LIMIT).round();
    commands.entity(change.source).insert(SliderValue(value));
    match rate_for_slider_value(value) {
        Some(rate) => {
            sim_commands.push(SimCommand::SetRate(rate));
            sim_commands.push(SimCommand::Play);
        }
        None => sim_commands.push(SimCommand::Pause),
    }
}

fn update_time_edits(
    focus: Res<InputFocus>,
    clock: Res<SimulationClock>,
    mut state: ResMut<TimeEditFocus>,
    mut fields: Query<(Entity, &TimeEditFieldComponent, &mut EditableText)>,
    mut commands: ResMut<SimCommandQueue>,
) {
    let focused = focus.get().filter(|entity| fields.get(*entity).is_ok());
    let mut presentation_t = clock.0.t();
    let mut cancelled_entity = None;

    if state.active != focused {
        if let Some(previous) = state.active {
            if let Ok((_entity, field, mut editable)) = fields.get_mut(previous) {
                if state.cancelled == Some(previous) {
                    let original_t_s = state.original_t_s.unwrap_or(presentation_t);
                    replace_editable_text(&mut editable, &display_value(field.field, original_t_s));
                    cancelled_entity = Some(previous);
                } else {
                    let outcome = commit_time_edit(
                        presentation_t,
                        field.field,
                        &editable.value().to_string(),
                    );
                    presentation_t = outcome.t_s;
                    if outcome.accepted {
                        commands.push(SimCommand::SetTime(outcome.t_s));
                    }
                    replace_editable_text(&mut editable, &outcome.display);
                }
            }
        }
        state.cancelled = None;
        if let Some(current) = focused {
            if let Ok((_entity, field, mut editable)) = fields.get_mut(current) {
                replace_editable_text(&mut editable, &edit_value(field.field, presentation_t));
            }
        }
        state.active = focused;
        state.original_t_s = focused.map(|_| presentation_t);
    }

    for (entity, field, mut editable) in &mut fields {
        if Some(entity) != state.active && Some(entity) != cancelled_entity {
            replace_editable_text(&mut editable, &display_value(field.field, presentation_t));
        }
    }
}

fn replace_editable_text(editable: &mut EditableText, replacement: &str) {
    if editable.value() != replacement {
        editable.queue_edit(TextEdit::SelectAll);
        editable.queue_edit(TextEdit::Insert(replacement.into()));
    }
}

fn spawn_slider_detents(
    mut commands: Commands,
    theme: Res<UiTheme>,
    sliders: Query<Entity, Added<TimeRateSlider>>,
) {
    for slider in &sliders {
        for detent in -12_i8..=12 {
            let position = f32::from(detent + 12) / 24.0 * 100.0;
            let color = if detent == 0 {
                theme.colors.accent.color()
            } else {
                theme.colors.separator.color()
            };
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(position),
                    top: px(7),
                    width: px(1),
                    height: px(if detent == 0 { 7 } else { 4 }),
                    ..default()
                },
                BackgroundColor(color),
                ChildOf(slider),
                Pickable::IGNORE,
                TimeSliderDetent,
            ));
        }
    }
}

fn sync_time_playback_controls(
    clock: Res<SimulationClock>,
    theme: Res<UiTheme>,
    mut commands: Commands,
    mut rate_labels: Query<&mut Text, (With<TimeRateLabel>, Without<PlayPauseGlyph>)>,
    sliders: Query<(Entity, &SliderValue, &SliderDragState), With<TimeRateSlider>>,
    mut play_buttons: Query<&mut BackgroundColor, With<PlayPauseButton>>,
    mut play_glyphs: Query<&mut Text, (With<PlayPauseGlyph>, Without<TimeRateLabel>)>,
) {
    // Do not reintroduce overlapping mutable `Text` queries here: Bevy rejects
    // the system at runtime even though these marker sets are logically disjoint.
    let playing = clock.0.is_playing();
    let slider_value = if playing {
        slider_value_for_rate(clock.0.rate())
    } else {
        0.0
    };
    for mut label in &mut rate_labels {
        **label = if playing {
            clock.0.rate().label()
        } else {
            "PAUSED".to_string()
        };
    }
    for (entity, value, drag) in &sliders {
        if !drag.dragging && value.0 != slider_value {
            commands.entity(entity).insert(SliderValue(slider_value));
        }
    }
    for mut background in &mut play_buttons {
        background.0 = if playing {
            with_alpha(theme.colors.accent, 40)
        } else {
            theme.colors.panel.color()
        };
    }
    for mut glyph in &mut play_glyphs {
        **glyph = if playing { "Ⅱ" } else { "▶" }.to_string();
    }
}

fn sync_live_chip(
    clock: Res<SimulationClock>,
    theme: Res<UiTheme>,
    mut live_chips: Query<(&mut BackgroundColor, &mut BorderColor), With<LiveChip>>,
    mut live_dots: Query<&mut BackgroundColor, (With<LiveDot>, Without<LiveChip>)>,
    mut live_text: Query<&mut TextColor, With<LiveText>>,
) {
    let is_live = live_chip_active(&clock.0, wall_now_t());
    let dot_color = if is_live {
        theme.colors.status_live.color()
    } else {
        theme.colors.text_disabled.color()
    };
    for (mut background, mut border) in &mut live_chips {
        background.0 = if is_live {
            with_alpha(theme.colors.status_live, 28)
        } else {
            theme.colors.background.color()
        };
        *border = BorderColor::all(if is_live {
            theme.colors.status_live.color()
        } else {
            theme.colors.separator.color()
        });
    }
    for mut dot in &mut live_dots {
        dot.0 = dot_color;
    }
    for mut text in &mut live_text {
        text.0 = if is_live {
            theme.colors.status_live.color()
        } else {
            theme.colors.text_disabled.color()
        };
    }
}

fn update_slider_thumb(
    sliders: Query<(Entity, &SliderValue), With<TimeRateSlider>>,
    children: Query<&Children>,
    mut thumbs: Query<&mut Node, With<TimeSliderThumb>>,
) {
    for (slider, value) in &sliders {
        let position = (value.0 + SLIDER_LIMIT) / (2.0 * SLIDER_LIMIT) * 100.0;
        for descendant in children.iter_descendants(slider) {
            if let Ok(mut thumb) = thumbs.get_mut(descendant) {
                thumb.left = percent(position);
            }
        }
    }
}

fn consume_tick_reports(
    mut reports: MessageReader<ClockTickReport>,
    stacks: Query<Entity, With<TimeToastStack>>,
    theme: Res<UiTheme>,
    mut commands: Commands,
) {
    let Ok(stack) = stacks.single() else {
        reports.clear();
        return;
    };
    for report in reports.read() {
        for notice in toasts_for_tick_report(report.0) {
            spawn_time_toast(&mut commands, stack, *theme, notice);
        }
    }
}

fn consume_orbit_emphasis_onsets(
    mut onsets: MessageReader<OrbitEmphasisOnset>,
    stacks: Query<Entity, With<TimeToastStack>>,
    theme: Res<UiTheme>,
    mut commands: Commands,
) {
    let Ok(stack) = stacks.single() else {
        onsets.clear();
        return;
    };
    for _onset in onsets.read() {
        spawn_time_toast(&mut commands, stack, *theme, TimeToastKind::OrbitEmphasis);
    }
}

fn spawn_time_toast(commands: &mut Commands, stack: Entity, theme: UiTheme, notice: TimeToastKind) {
    commands
        .spawn_scene(toast(
            theme,
            WidgetSpec::new(
                notice.text(),
                format!("Simulation notice: {}", notice.text()),
                WidgetVisualState::Default,
            ),
        ))
        .insert((
            ChildOf(stack),
            TimeToast {
                remaining_s: TOAST_LIFETIME_S,
            },
        ));
}

fn expire_time_toasts(
    time: Res<Time>,
    mut commands: Commands,
    mut toasts: Query<(Entity, &mut TimeToast)>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.remaining_s -= time.delta_secs();
        if toast.remaining_s <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn with_alpha(token: UiColorToken, alpha: u8) -> Color {
    Color::srgba_u8(token.0[0], token.0[1], token.0[2], alpha)
}

fn edit_value(field: TimeEditField, t_s: f64) -> String {
    let datetime = datetime_from_t(t_s);
    match field {
        TimeEditField::Date => format!(
            "{:04}-{:02}-{:02}",
            datetime.year, datetime.month, datetime.day
        ),
        TimeEditField::Clock => format_clock(datetime),
    }
}

fn display_value(field: TimeEditField, t_s: f64) -> String {
    let datetime = datetime_from_t(t_s);
    match field {
        TimeEditField::Date => format_date_eyes(&datetime),
        TimeEditField::Clock => format_clock(datetime),
    }
}

fn format_clock(datetime: DateTime) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        datetime.hour, datetime.minute, datetime.second
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        input::{keyboard::Key, InputPlugin},
        input_focus::{FocusCause, InputDispatchPlugin, InputFocusPlugin},
        window::PrimaryWindow,
    };
    use sim_core::time::{t_from_jd_tdb, StartMode};
    use std::collections::HashMap;

    fn time_edit_keypress(
        field: TimeEditField,
        replacement: &str,
        key_code: KeyCode,
        logical_key: Key,
    ) -> (String, Vec<SimCommand>, Option<Entity>) {
        let original_clock = SimClock::new(StartMode::default(), 0.0);
        let expected_display = display_value(field, original_clock.t());
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            InputPlugin,
            InputFocusPlugin,
            InputDispatchPlugin,
        ))
        .insert_resource(SimulationClock(original_clock))
        .init_resource::<TimeEditFocus>()
        .init_resource::<SimCommandQueue>()
        .add_systems(Update, update_time_edits);
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let editable = app
            .world_mut()
            .spawn((
                TimeEditFieldComponent { field },
                EditableText::new(expected_display.clone()),
            ))
            .observe(finish_time_edit)
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(editable, FocusCause::Navigated);
        app.update();
        {
            let world = app.world_mut();
            let mut entity = world.entity_mut(editable);
            let mut value = entity.get_mut::<EditableText>().unwrap();
            value.editor_mut().set_text(replacement);
            value.pending_edits.clear();
        }
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        app.update();

        let queued_replacement = app
            .world()
            .entity(editable)
            .get::<EditableText>()
            .unwrap()
            .pending_edits
            .iter()
            .find_map(|edit| match edit {
                TextEdit::Insert(value) => Some(value.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| {
                app.world()
                    .entity(editable)
                    .get::<EditableText>()
                    .unwrap()
                    .value()
                    .to_string()
            });
        let commands = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        let focus = app.world().resource::<InputFocus>().get();
        (queued_replacement, commands, focus)
    }

    #[test]
    fn slider_round_trips_every_rate_index_detent_through_wp1_mapping() {
        let expected: Vec<_> = RateIndex::detents().collect();
        let actual: Vec<_> = expected
            .iter()
            .map(|rate| rate_for_slider_value(slider_value_for_rate(*rate)).unwrap())
            .collect();

        assert_eq!(actual, expected);
        assert_eq!(rate_for_slider_value(0.0), None);
    }

    #[test]
    fn invalid_date_and_time_edits_revert_without_moving_clock() {
        let original_t = t_from_jd_tdb(2_461_233.0);
        let invalid_date = commit_time_edit(original_t, TimeEditField::Date, "2026-02-30");
        let invalid_time = commit_time_edit(original_t, TimeEditField::Clock, "24:00:00");

        assert!(!invalid_date.accepted);
        assert!(!invalid_time.accepted);
        assert_eq!(invalid_date.t_s.to_bits(), original_t.to_bits());
        assert_eq!(invalid_time.t_s.to_bits(), original_t.to_bits());
        assert_eq!(invalid_date.display, "JUL 11, 2026");
        assert_eq!(invalid_time.display, "12:00:00");
    }

    #[test]
    fn escape_reverts_date_and_clock_edits_without_a_command() {
        for (field, replacement) in [
            (TimeEditField::Date, "2026-01-02"),
            (TimeEditField::Clock, "13:00:00"),
        ] {
            let expected = display_value(field, SimClock::new(StartMode::default(), 0.0).t());
            let (display, commands, focus) =
                time_edit_keypress(field, replacement, KeyCode::Escape, Key::Escape);
            assert_eq!(display, expected);
            assert!(commands.is_empty());
            assert_eq!(focus, None);
        }
    }

    #[test]
    fn enter_commits_one_set_time_and_no_background_gameplay_command() {
        let (_display, commands, focus) = time_edit_keypress(
            TimeEditField::Date,
            "2026-01-02",
            KeyCode::Enter,
            Key::Enter,
        );
        assert_eq!(commands.len(), 1);
        assert!(matches!(commands[0], SimCommand::SetTime(_)));
        assert_eq!(focus, None);
    }

    #[test]
    fn live_chip_matches_clock_predicate_in_all_four_regimes() {
        let now = t_from_jd_tdb(2_461_233.0);

        let mut paused = SimClock::new(StartMode::Live, now);
        paused.pause();
        let mut wrong_rate = SimClock::new(StartMode::Live, now);
        wrong_rate.set_rate(RateIndex::new(2).unwrap());
        let mut snapping = SimClock::new(StartMode::default(), now);
        snapping.snap_to_live();
        let live = SimClock::new(StartMode::Live, now);

        for clock in [&paused, &wrong_rate, &snapping, &live] {
            assert_eq!(live_chip_active(clock, now), clock.is_live(now));
        }
        assert!(!live_chip_active(&paused, now));
        assert!(!live_chip_active(&wrong_rate, now));
        assert!(!live_chip_active(&snapping, now));
        assert!(live_chip_active(&live, now));
    }

    #[test]
    fn synthetic_transition_reports_create_exactly_one_toast_per_event() {
        let reports = [
            TickReport::default(),
            TickReport {
                clamped: Some(RangeEdge::AtMin),
                ..default()
            },
            TickReport::default(),
            TickReport::default(),
            TickReport {
                extrapolation_changed: Some(true),
                ..default()
            },
            TickReport::default(),
            TickReport {
                extrapolation_changed: Some(false),
                ..default()
            },
            TickReport {
                clamped: Some(RangeEdge::AtMin),
                ..default()
            },
            TickReport {
                snapped_live: true,
                ..default()
            },
        ];
        let mut counts = HashMap::new();
        for notice in reports.into_iter().flat_map(toasts_for_tick_report) {
            *counts.entry(notice).or_insert(0) += 1;
        }

        assert_eq!(counts.get(&TimeToastKind::RangeMinimum), Some(&2));
        assert_eq!(counts.get(&TimeToastKind::Extrapolation), Some(&1));
        assert_eq!(counts.get(&TimeToastKind::SnappedLive), Some(&1));
        assert_eq!(counts.get(&TimeToastKind::RangeMaximum), None);
    }
}
