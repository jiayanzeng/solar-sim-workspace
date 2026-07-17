//! WP8 Eyes-style time bar — Rev C §§4.2, 7, and 9.5–9.6.
//!
//! Presentation binds directly to WP1's clock API. Edits and controls enqueue
//! `SimCommand`s, while toasts consume transition-only `TickReport`s; neither
//! path reimplements clock levels or time arithmetic.

use crate::control::{SimCommand, SimCommandQueue};
use crate::layers::HudSurface;
use crate::scene_polish::OrbitEmphasisSet;
use crate::ui_kit::{toast, UiColorToken, UiTheme, WidgetSpec, WidgetVisualState};
use crate::{
    wall_now_t, ClockTickReport, OrbitEmphasisOnset, SimulationClock, SimulationSet,
    INTER_FONT_ASSET,
};
use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        tab_navigation::{TabGroup, TabIndex, TabNavigationPlugin},
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
                    consume_orbit_emphasis_onsets.after(OrbitEmphasisSet),
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
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: px(theme.spacing.xs_px),
        }
        TimeBarRoot
        HudSurface
        AccessibleLabel("Simulation time bar")
        template_value(TabGroup::new(20))
        BackgroundColor({theme.colors.top_bar.color()})
        BorderColor::all(theme.colors.separator.color())
        GlobalZIndex(95)
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(38),
                    align_items: AlignItems::Center,
                    column_gap: px(theme.spacing.sm_px),
                }
                Children [
                    edit_field(
                        theme,
                        TimeEditField::Date,
                        format_date_eyes(&datetime),
                        "Simulation date",
                    ),
                    edit_field(
                        theme,
                        TimeEditField::Clock,
                        format_clock(datetime),
                        "Simulation clock",
                    ),
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
                            min_width: px(0),
                        }
                    ),
                    live_chip(theme),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.spacing.xs_px),
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
        ]
    }
}

fn edit_field(
    theme: UiTheme,
    field: TimeEditField,
    value: String,
    accessible_label: &'static str,
) -> impl Scene {
    let (width, min_width) = match field {
        TimeEditField::Date => (154.0, 110.0),
        TimeEditField::Clock => (92.0, 72.0),
    };
    bsn! {
        Node {
            width: px(width),
            min_width: px(min_width),
            height: px(38),
            flex_shrink: 1.0,
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
            template_value(TabIndex(match field {
                TimeEditField::Date => 0,
                TimeEditField::Clock => 1,
            }))
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
        TabIndex(2)
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
        TabIndex(4)
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
        TabIndex(3)
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
            right: px(theme.spacing.lg_px),
            bottom: px(TIME_BAR_HEIGHT_PX + theme.spacing.lg_px),
            width: auto(),
            max_width: px(390),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.spacing.sm_px),
        }
        TimeToastStack
        HudSurface
        AccessibleLabel("Simulation notices")
        Pickable::IGNORE
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
    let rate_label = if playing {
        clock.0.rate().label()
    } else {
        "PAUSED".to_string()
    };
    for mut label in &mut rate_labels {
        if label.as_str() != rate_label {
            **label = rate_label.clone();
        }
    }
    for (entity, value, drag) in &sliders {
        if !drag.dragging && value.0 != slider_value {
            commands.entity(entity).insert(SliderValue(slider_value));
        }
    }
    let desired_play_background = if playing {
        with_alpha(theme.colors.accent, 40)
    } else {
        theme.colors.panel.color()
    };
    for mut background in &mut play_buttons {
        if background.0 != desired_play_background {
            background.0 = desired_play_background;
        }
    }
    let desired_glyph = if playing { "Ⅱ" } else { "▶" };
    for mut glyph in &mut play_glyphs {
        if glyph.as_str() != desired_glyph {
            **glyph = desired_glyph.to_string();
        }
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
    let desired_chip_background = if is_live {
        with_alpha(theme.colors.status_live, 28)
    } else {
        theme.colors.background.color()
    };
    let desired_chip_border = BorderColor::all(if is_live {
        theme.colors.status_live.color()
    } else {
        theme.colors.separator.color()
    });
    for (mut background, mut border) in &mut live_chips {
        if background.0 != desired_chip_background {
            background.0 = desired_chip_background;
        }
        if *border != desired_chip_border {
            *border = desired_chip_border;
        }
    }
    for mut dot in &mut live_dots {
        if dot.0 != dot_color {
            dot.0 = dot_color;
        }
    }
    let desired_text_color = if is_live {
        theme.colors.status_live.color()
    } else {
        theme.colors.text_disabled.color()
    };
    for mut text in &mut live_text {
        if text.0 != desired_text_color {
            text.0 = desired_text_color;
        }
    }
}

type ChangedTimeRateSliders<'w, 's> =
    Query<'w, 's, (Entity, &'static SliderValue), (With<TimeRateSlider>, Changed<SliderValue>)>;

fn update_slider_thumb(
    sliders: ChangedTimeRateSliders,
    children: Query<&Children>,
    mut thumbs: Query<&mut Node, With<TimeSliderThumb>>,
) {
    for (slider, value) in &sliders {
        let position = (value.0 + SLIDER_LIMIT) / (2.0 * SLIDER_LIMIT) * 100.0;
        for descendant in children.iter_descendants(slider) {
            if let Ok(mut thumb) = thumbs.get_mut(descendant) {
                let desired = percent(position);
                if thumb.left != desired {
                    thumb.left = desired;
                }
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
    use crate::ui_kit::test_layout;
    use crate::{load_catalog_text, LoadedCatalog, ScenePolishPlugin, SimulationTickAdvance};
    use bevy::{
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        camera::{NormalizedRenderTarget, RenderTarget},
        input::{keyboard::Key, InputPlugin},
        input_focus::{FocusCause, InputDispatchPlugin, InputFocusPlugin},
        picking::{
            hover::HoverMap,
            pointer::{Location, PointerId, PointerLocation},
            InteractionPlugin, PickingPlugin,
        },
        scene::{ScenePlugin, WorldSceneExt},
        text::TextLayoutInfo,
        ui::UiStack,
        window::PrimaryWindow,
    };
    use sim_core::time::{t_from_jd_tdb, StartMode};
    use std::collections::HashMap;
    use std::time::Duration;

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    #[derive(Resource, Debug, Default)]
    struct TimeControlWrites {
        texts: usize,
        backgrounds: usize,
        borders: usize,
        text_colors: usize,
        nodes: usize,
    }

    type ChangedTimeControlTexts<'w, 's> = Query<
        'w,
        's,
        Entity,
        (
            Changed<Text>,
            Or<(With<TimeRateLabel>, With<PlayPauseGlyph>)>,
        ),
    >;
    type ChangedTimeControlBackgrounds<'w, 's> = Query<
        'w,
        's,
        Entity,
        (
            Changed<BackgroundColor>,
            Or<(With<PlayPauseButton>, With<LiveChip>, With<LiveDot>)>,
        ),
    >;

    fn count_time_control_writes(
        texts: ChangedTimeControlTexts,
        backgrounds: ChangedTimeControlBackgrounds,
        borders: Query<Entity, (Changed<BorderColor>, With<LiveChip>)>,
        text_colors: Query<Entity, (Changed<TextColor>, With<LiveText>)>,
        nodes: Query<Entity, (Changed<Node>, With<TimeSliderThumb>)>,
        mut writes: ResMut<TimeControlWrites>,
    ) {
        writes.texts = texts.iter().count();
        writes.backgrounds = backgrounds.iter().count();
        writes.borders = borders.iter().count();
        writes.text_colors = text_colors.iter().count();
        writes.nodes = nodes.iter().count();
    }

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

    fn emphasis_toast_app() -> App {
        let loaded = LoadedCatalog::new(load_catalog_text(REAL_CATALOG).unwrap());
        let t_s = t_from_jd_tdb(2_461_042.0);
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Time<Real>>()
        .init_resource::<SimulationTickAdvance>()
        .insert_resource(loaded)
        .insert_resource(UiTheme::default())
        .insert_resource(SimulationClock(SimClock::new(
            StartMode::FixedEpoch {
                jd_tdb: 2_461_042.0,
            },
            t_s,
        )))
        .add_plugins(ScenePolishPlugin)
        .add_systems(
            Update,
            consume_orbit_emphasis_onsets
                .in_set(SimulationSet::Render)
                .after(OrbitEmphasisSet),
        );
        app.world_mut().spawn(TimeToastStack);
        advance_emphasis_toast_frame(&mut app, 0.0, 0.0);
        app
    }

    fn advance_emphasis_toast_frame(app: &mut App, simulated_step_s: f64, wall_delta_s: f64) {
        app.world_mut()
            .resource_mut::<SimulationTickAdvance>()
            .seconds = simulated_step_s;
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs_f64(wall_delta_s));
        app.update();
    }

    fn orbit_emphasis_toast_count(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<TimeToast>>()
            .iter(world)
            .count()
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
    fn stable_time_controls_do_not_rewrite_components() {
        let theme = UiTheme::default();
        let mut clock = SimClock::new(StartMode::default(), 0.0);
        clock.pause();
        let mut app = App::new();
        app.insert_resource(theme)
            .insert_resource(SimulationClock(clock))
            .init_resource::<TimeControlWrites>()
            .add_systems(
                Update,
                (
                    sync_time_playback_controls,
                    sync_live_chip,
                    update_slider_thumb,
                    count_time_control_writes,
                )
                    .chain(),
            );
        app.world_mut().spawn((TimeRateLabel, Text::new("PAUSED")));
        app.world_mut()
            .spawn((PlayPauseButton, BackgroundColor(theme.colors.panel.color())));
        app.world_mut().spawn((PlayPauseGlyph, Text::new("▶")));
        app.world_mut().spawn((
            LiveChip,
            BackgroundColor(theme.colors.background.color()),
            BorderColor::all(theme.colors.separator.color()),
        ));
        app.world_mut()
            .spawn((LiveDot, BackgroundColor(theme.colors.text_disabled.color())));
        app.world_mut()
            .spawn((LiveText, TextColor(theme.colors.text_disabled.color())));
        let slider = app
            .world_mut()
            .spawn((TimeRateSlider, SliderValue(0.0), SliderDragState::default()))
            .id();
        app.world_mut().spawn((
            TimeSliderThumb,
            Node {
                left: percent(50),
                ..default()
            },
            ChildOf(slider),
        ));
        app.update();

        *app.world_mut().resource_mut::<TimeControlWrites>() = TimeControlWrites::default();
        app.update();

        let writes = app.world().resource::<TimeControlWrites>();
        assert_eq!(writes.texts, 0);
        assert_eq!(writes.backgrounds, 0);
        assert_eq!(writes.borders, 0);
        assert_eq!(writes.text_colors, 0);
        assert_eq!(writes.nodes, 0);
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

    #[test]
    fn toast_expiry_despawns_each_notice_once_at_its_own_deadline() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_systems(Update, expire_time_toasts);
        let first = app.world_mut().spawn(TimeToast { remaining_s: 0.5 }).id();
        let second = app.world_mut().spawn(TimeToast { remaining_s: 1.5 }).id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(1));
        app.world_mut().run_schedule(Update);
        assert!(app.world().get_entity(first).is_err());
        assert!(app.world().get_entity(second).is_ok());

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(1));
        app.world_mut().run_schedule(Update);
        assert!(app.world().get_entity(second).is_err());

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs(1));
        app.world_mut().run_schedule(Update);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<TimeToast>>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn orbit_emphasis_messages_create_one_same_frame_toast_per_global_onset() {
        const FRAME_S: f64 = 1.0 / 60.0;

        let mut app = emphasis_toast_app();
        assert_eq!(orbit_emphasis_toast_count(&mut app), 0);

        let hundred_year_step = RateIndex::MAX.seconds_per_second() * FRAME_S;
        advance_emphasis_toast_frame(&mut app, hundred_year_step, FRAME_S);
        assert_eq!(
            orbit_emphasis_toast_count(&mut app),
            1,
            "the onset emitted in OrbitEmphasisSet must be consumed in the same frame"
        );

        for _ in 0..8 {
            advance_emphasis_toast_frame(&mut app, hundred_year_step, FRAME_S);
        }
        assert_eq!(
            orbit_emphasis_toast_count(&mut app),
            1,
            "a held emphasis level must not emit per-frame toasts"
        );

        advance_emphasis_toast_frame(&mut app, FRAME_S, FRAME_S);
        assert_eq!(orbit_emphasis_toast_count(&mut app), 1);

        advance_emphasis_toast_frame(&mut app, hundred_year_step, FRAME_S);
        assert_eq!(
            orbit_emphasis_toast_count(&mut app),
            2,
            "release followed by re-entry is exactly one new onset"
        );
    }

    #[test]
    fn time_controls_fit_every_required_viewport_and_scale() {
        for (width, height, scale) in test_layout::required_viewports() {
            let mut app = test_layout::app(width, height, scale);
            app.insert_resource(UiTheme::default())
                .insert_resource(SimulationClock(SimClock::new(StartMode::default(), 0.0)))
                .add_systems(Startup, spawn_time_bar);
            test_layout::settle(&mut app);

            let world = app.world_mut();
            let root = world
                .query_filtered::<Entity, With<TimeBarRoot>>()
                .single(world)
                .unwrap();
            let group = world.get::<TabGroup>(root).unwrap();
            assert_eq!(group.order, 20);
            assert!(!group.modal);
            let root_rect = node_rect(world, root);
            let mut controls = world.query_filtered::<Entity, Or<(
                With<TimeEditFieldComponent>,
                With<PlayPauseButton>,
                With<TimeRateSlider>,
                With<LiveChip>,
            )>>();
            let controls: Vec<_> = controls.iter(world).collect();
            assert_eq!(controls.len(), 5);
            for entity in controls {
                let rect = node_rect(world, entity);
                assert!(
                    rect.min.x >= root_rect.min.x - 1.0
                        && rect.max.x <= root_rect.max.x + 1.0
                        && rect.min.y >= root_rect.min.y - 1.0
                        && rect.max.y <= root_rect.max.y + 1.0,
                    "{width}×{height} scale {scale}: time control {entity:?} {rect:?} escaped {root_rect:?}"
                );
            }

            let mut indices = world.query_filtered::<&TabIndex, Or<(
                With<TimeEditFieldComponent>,
                With<PlayPauseButton>,
                With<TimeRateSlider>,
                With<LiveChip>,
            )>>();
            let mut indices: Vec<_> = indices.iter(world).map(|index| index.0).collect();
            indices.sort_unstable();
            assert_eq!(indices, vec![0, 1, 2, 3, 4]);
        }
    }

    #[test]
    fn active_toasts_stay_bounded_wrapped_and_separate_at_required_viewports() {
        const LONG_NOTICE: &str = "Orbit paths are simplified outside the supported interval; return to the supported interval for full orbital precision.";

        for (width, height, scale) in test_layout::required_viewports() {
            let theme = UiTheme::default();
            let mut app = test_layout::app(width, height, scale);
            app.insert_resource(theme)
                .insert_resource(SimulationClock(SimClock::new(StartMode::default(), 0.0)))
                .add_systems(Startup, spawn_time_bar);
            test_layout::settle(&mut app);

            let stack = app
                .world_mut()
                .query_filtered::<Entity, With<TimeToastStack>>()
                .single(app.world())
                .unwrap();
            let toast_entity = app
                .world_mut()
                .spawn_scene(toast(
                    theme,
                    WidgetSpec::new(
                        LONG_NOTICE,
                        format!("Simulation notice: {LONG_NOTICE}"),
                        WidgetVisualState::Default,
                    ),
                ))
                .unwrap()
                .insert((
                    ChildOf(stack),
                    TimeToast {
                        remaining_s: TOAST_LIFETIME_S,
                    },
                ))
                .id();
            let second_toast = app
                .world_mut()
                .spawn_scene(toast(
                    theme,
                    WidgetSpec::new(
                        "Simulation returned to the supported interval.",
                        "Simulation notice: returned to the supported interval",
                        WidgetVisualState::Default,
                    ),
                ))
                .unwrap()
                .insert((
                    ChildOf(stack),
                    TimeToast {
                        remaining_s: TOAST_LIFETIME_S,
                    },
                ))
                .id();
            test_layout::settle(&mut app);

            let world = app.world_mut();
            let time_bar = world
                .query_filtered::<Entity, With<TimeBarRoot>>()
                .single(world)
                .unwrap();
            let text_entity = world
                .get::<Children>(toast_entity)
                .unwrap()
                .iter()
                .find(|entity| world.get::<Text>(*entity).is_some())
                .unwrap();
            let stack_rect = node_rect(world, stack);
            let toast_rect = node_rect(world, toast_entity);
            let second_toast_rect = node_rect(world, second_toast);
            let text_rect = node_rect(world, text_entity);
            let time_bar_rect = node_rect(world, time_bar);
            let inset_px = theme.spacing.lg_px * scale;
            let expected_stack_width = (390.0 * scale).min(width as f32 - 2.0 * inset_px);

            assert!(
                (stack_rect.width() - expected_stack_width).abs() <= 1.0,
                "{width}×{height} scale {scale}: stack {stack_rect:?} did not resolve to {expected_stack_width}px"
            );
            assert!(
                stack_rect.min.x >= inset_px - 1.0
                    && stack_rect.max.x <= width as f32 - inset_px + 1.0,
                "{width}×{height} scale {scale}: stack {stack_rect:?} escaped the viewport inset"
            );
            assert!(
                toast_rect.min.x >= stack_rect.min.x - 1.0
                    && toast_rect.max.x <= stack_rect.max.x + 1.0
                    && text_rect.min.x >= toast_rect.min.x - 1.0
                    && text_rect.max.x <= toast_rect.max.x + 1.0,
                "{width}×{height} scale {scale}: toast/text {toast_rect:?}/{text_rect:?} escaped {stack_rect:?}"
            );
            assert!(
                second_toast_rect.max.y <= time_bar_rect.min.y + 1.0,
                "{width}×{height} scale {scale}: toast {second_toast_rect:?} overlapped time bar {time_bar_rect:?}"
            );
            assert!(
                (second_toast_rect.min.y
                    - toast_rect.max.y
                    - theme.spacing.sm_px * scale)
                    .abs()
                    <= 1.0,
                "{width}×{height} scale {scale}: toasts {toast_rect:?}/{second_toast_rect:?} lost their theme spacing"
            );
            let text_layout = world.get::<TextLayoutInfo>(text_entity).unwrap();
            assert!(
                text_layout.size.x * scale <= toast_rect.width() + 1.0
                    && text_layout.size.y > theme.type_scale.body_px,
                "{width}×{height} scale {scale}: toast text layout {:?} did not wrap within {toast_rect:?}",
                text_layout.size
            );
            assert_eq!(world.get::<Pickable>(stack), Some(&Pickable::IGNORE));
            assert_eq!(world.get::<Pickable>(toast_entity), Some(&Pickable::IGNORE));
            assert_eq!(world.get::<Pickable>(text_entity), Some(&Pickable::IGNORE));
        }
    }

    #[test]
    fn ui_picking_reaches_the_underlying_target_through_an_active_toast() {
        let theme = UiTheme::default();
        let mut app = test_layout::app(800, 600, 1.0);
        app.add_plugins((PickingPlugin, InteractionPlugin));
        let target = app
            .world_mut()
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                Pickable::default(),
                GlobalZIndex(100),
            ))
            .id();
        let stack = app
            .world_mut()
            .spawn_scene(toast_stack(theme))
            .unwrap()
            .id();
        let toast_entity = app
            .world_mut()
            .spawn_scene(toast(
                theme,
                WidgetSpec::new(
                    "Simulation notice",
                    "Simulation notice: test",
                    WidgetVisualState::Default,
                ),
            ))
            .unwrap()
            .insert((
                ChildOf(stack),
                TimeToast {
                    remaining_s: TOAST_LIFETIME_S,
                },
            ))
            .id();
        test_layout::settle(&mut app);

        let toast_center = node_rect(app.world(), toast_entity).center();
        let camera = app
            .world_mut()
            .query_filtered::<Entity, With<Camera2d>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .entity_mut(camera)
            .insert(RenderTarget::None {
                size: UVec2::new(800, 600),
            });
        let node_entities: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<Node>>()
            .iter(app.world())
            .collect();
        for entity in node_entities {
            app.world_mut()
                .entity_mut(entity)
                .insert(InheritedVisibility::VISIBLE);
        }
        app.world_mut().spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target: NormalizedRenderTarget::None {
                    width: 800,
                    height: 600,
                },
                position: toast_center,
            }),
        ));
        let text_entity = app
            .world()
            .get::<Children>(toast_entity)
            .unwrap()
            .iter()
            .find(|entity| app.world().get::<Text>(*entity).is_some())
            .unwrap();
        app.world_mut().insert_resource(UiStack {
            partition: std::iter::once(0..4).collect(),
            uinodes: vec![target, stack, toast_entity, text_entity],
        });
        app.world_mut().run_schedule(First);
        app.world_mut().run_schedule(PreUpdate);

        let hovered = app
            .world()
            .resource::<HoverMap>()
            .get(&PointerId::Mouse)
            .unwrap();
        assert!(hovered.contains_key(&target));
        assert!(!hovered.contains_key(&stack));
        assert!(!hovered.contains_key(&toast_entity));
        assert!(!hovered.contains_key(&text_entity));
    }

    fn node_rect(world: &World, entity: Entity) -> Rect {
        let node = world.get::<ComputedNode>(entity).unwrap();
        let center = world
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .affine()
            .translation;
        Rect::from_center_size(center, node.size())
    }
}
