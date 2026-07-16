//! WP12 instant catalog search and full-screen browse menu — Rev C §§4.1, 9.1.
//!
//! Search starts with `Catalog::find`, preserving the core's exact contract,
//! then adds deterministic prefix, alias, and fuzzy candidates. Presentation
//! owns only edit/dropdown/menu state; every selection crosses the existing
//! `SimCommand::TravelToBody` boundary.

use crate::control::{SimCommand, SimCommandQueue};
use crate::input_intent::UiScrollSurface;
use crate::layers::HudSurface;
use crate::ui_kit::{
    MenuBrowseButton, SearchHint, SearchInput, UiTheme, INTER_FONT_ASSET, TOP_BAR_HEIGHT_PX,
};
use crate::{LoadedCatalog, SimulationSet};
use bevy::{
    input::mouse::MouseScrollUnit,
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, FocusedInput, InputFocus,
    },
    prelude::*,
    text::{EditableText, LetterSpacing, LineBreak, TextEdit, TextLayout},
    ui_widgets::Activate,
};
use sim_core::catalog::{BodyRecord, Catalog, Category};

const SEARCH_DROPDOWN_WIDTH_PX: f32 = 360.0;
const SEARCH_DROPDOWN_Z_INDEX: i32 = 112;
const MENU_Z_INDEX: i32 = 114;
const MAX_DROPDOWN_RESULTS: usize = 8;

const PLANETS_AND_MOONS_SHORTLIST: &[&str] = &[
    "sun", "mercury", "venus", "earth", "moon", "mars", "jupiter", "io", "europa", "saturn",
    "titan", "uranus", "neptune",
];
const DWARFS_AND_ASTEROIDS_SHORTLIST: &[&str] = &[
    "ceres", "pluto", "eris", "haumea", "makemake", "sedna", "vesta", "psyche",
];
const COMETS_SHORTLIST: &[&str] = &["halley", "hale_bopp", "churyumov_gerasimenko", "3i_atlas"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SearchMatchKind {
    Exact,
    Prefix,
    Alias,
    Fuzzy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub body_index: usize,
    pub body_id: String,
    pub display_name: String,
    pub matched_key: String,
    pub kind: SearchMatchKind,
    score: usize,
}

impl SearchHit {
    fn sort_key(&self) -> (SearchMatchKind, usize, String, usize) {
        (
            self.kind,
            self.score,
            self.display_name.to_lowercase(),
            self.body_index,
        )
    }
}

/// Returns one best candidate per body. The exact result from `Catalog::find`
/// is injected first and cannot be displaced by later fuzzy scoring.
pub fn search_catalog(catalog: &Catalog, query: &str) -> Vec<SearchHit> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let normalized_query = query.to_lowercase();
    let exact_id = catalog.find(query).map(|body| body.id.as_str());
    let mut hits = Vec::new();

    for (body_index, body) in catalog.bodies.iter().enumerate() {
        if exact_id == Some(body.id.as_str()) {
            hits.push(make_hit(
                body_index,
                body,
                exact_matching_key(body, &normalized_query),
                SearchMatchKind::Exact,
                0,
            ));
            continue;
        }

        if let Some((key, score)) = best_prefix_match(body, &normalized_query) {
            hits.push(make_hit(
                body_index,
                body,
                key,
                SearchMatchKind::Prefix,
                score,
            ));
            continue;
        }
        if let Some((key, score)) = best_alias_match(body, &normalized_query) {
            hits.push(make_hit(
                body_index,
                body,
                key,
                SearchMatchKind::Alias,
                score,
            ));
            continue;
        }
        if let Some((key, score)) = best_fuzzy_match(body, &normalized_query) {
            hits.push(make_hit(
                body_index,
                body,
                key,
                SearchMatchKind::Fuzzy,
                score,
            ));
        }
    }

    hits.sort_by_key(SearchHit::sort_key);
    hits
}

fn make_hit(
    body_index: usize,
    body: &BodyRecord,
    matched_key: &str,
    kind: SearchMatchKind,
    score: usize,
) -> SearchHit {
    SearchHit {
        body_index,
        body_id: body.id.clone(),
        display_name: body.name.clone(),
        matched_key: matched_key.to_string(),
        kind,
        score,
    }
}

fn exact_matching_key<'a>(body: &'a BodyRecord, normalized_query: &str) -> &'a str {
    if body.name.to_lowercase() == normalized_query {
        return &body.name;
    }
    if body.id == normalized_query {
        return &body.id;
    }
    if let Some(designation) = body.designation.as_deref() {
        if designation.to_lowercase() == normalized_query {
            return designation;
        }
    }
    body.aliases
        .iter()
        .find(|alias| alias.to_lowercase() == normalized_query)
        .map_or(body.name.as_str(), String::as_str)
}

fn best_prefix_match<'a>(body: &'a BodyRecord, normalized_query: &str) -> Option<(&'a str, usize)> {
    let keys = std::iter::once(body.name.as_str()).chain(body.designation.as_deref());
    keys.filter_map(|key| {
        let normalized = key.to_lowercase();
        normalized.starts_with(normalized_query).then(|| {
            (
                key,
                normalized.chars().count() - normalized_query.chars().count(),
            )
        })
    })
    .min_by_key(|(_key, score)| *score)
}

fn best_alias_match<'a>(body: &'a BodyRecord, normalized_query: &str) -> Option<(&'a str, usize)> {
    body.aliases
        .iter()
        .filter_map(|alias| {
            fuzzy_score(normalized_query, &alias.to_lowercase())
                .map(|score| (alias.as_str(), score))
        })
        .min_by_key(|(_alias, score)| *score)
}

fn best_fuzzy_match<'a>(body: &'a BodyRecord, normalized_query: &str) -> Option<(&'a str, usize)> {
    std::iter::once(body.name.as_str())
        .chain(body.designation.as_deref())
        .filter_map(|key| {
            fuzzy_score(normalized_query, &key.to_lowercase()).map(|score| (key, score))
        })
        .min_by_key(|(_key, score)| *score)
}

fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let query_chars: Vec<char> = query.chars().collect();
    let candidate_chars: Vec<char> = candidate.chars().collect();
    if query_chars.is_empty() {
        return None;
    }

    if let Some(byte_index) = candidate.find(query) {
        let character_index = candidate[..byte_index].chars().count();
        return Some(
            character_index * 16 + candidate_chars.len().saturating_sub(query_chars.len()),
        );
    }

    if let Some((gap_count, tail)) = subsequence_gaps(&query_chars, &candidate_chars) {
        return Some(200 + gap_count * 8 + tail);
    }

    let threshold = (query_chars.len() / 3).clamp(1, 3);
    if query_chars.len() < 3 || query_chars.len().abs_diff(candidate_chars.len()) > threshold {
        return None;
    }
    let distance = edit_distance(&query_chars, &candidate_chars);
    (distance <= threshold).then_some(400 + distance * 32 + candidate_chars.len())
}

fn subsequence_gaps(query: &[char], candidate: &[char]) -> Option<(usize, usize)> {
    let mut candidate_index = 0;
    let mut gap_count = 0;
    for query_char in query {
        let offset = candidate[candidate_index..]
            .iter()
            .position(|candidate_char| candidate_char == query_char)?;
        gap_count += offset;
        candidate_index += offset + 1;
    }
    Some((gap_count, candidate.len().saturating_sub(query.len())))
}

fn edit_distance(left: &[char], right: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];
    for (left_index, left_char) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_char != right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BrowseCounts {
    pub stars: usize,
    pub planets: usize,
    pub dwarf_planets: usize,
    pub asteroids: usize,
    pub moons: usize,
    pub comets: usize,
}

impl BrowseCounts {
    pub fn from_catalog(catalog: &Catalog) -> Self {
        let counts = catalog.counts_by_category();
        let count = |category| counts.get(&category).copied().unwrap_or(0);
        Self {
            stars: count(Category::Star),
            planets: count(Category::Planet),
            dwarf_planets: count(Category::DwarfPlanet),
            asteroids: count(Category::Asteroid),
            moons: count(Category::Moon),
            comets: count(Category::Comet),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowseColumnKind {
    PlanetsAndMoons,
    DwarfsAndAsteroids,
    Comets,
}

impl BrowseColumnKind {
    const ALL: [Self; 3] = [
        Self::PlanetsAndMoons,
        Self::DwarfsAndAsteroids,
        Self::Comets,
    ];

    const fn title(self) -> &'static str {
        match self {
            Self::PlanetsAndMoons => "PLANETS & MOONS",
            Self::DwarfsAndAsteroids => "DWARF PLANETS & ASTEROIDS",
            Self::Comets => "COMETS",
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::PlanetsAndMoons => 0,
            Self::DwarfsAndAsteroids => 1,
            Self::Comets => 2,
        }
    }

    fn includes(self, category: Category) -> bool {
        match self {
            Self::PlanetsAndMoons => {
                matches!(category, Category::Star | Category::Planet | Category::Moon)
            }
            Self::DwarfsAndAsteroids => {
                matches!(category, Category::DwarfPlanet | Category::Asteroid)
            }
            Self::Comets => category == Category::Comet,
        }
    }

    const fn shortlist_ids(self) -> &'static [&'static str] {
        match self {
            Self::PlanetsAndMoons => PLANETS_AND_MOONS_SHORTLIST,
            Self::DwarfsAndAsteroids => DWARFS_AND_ASTEROIDS_SHORTLIST,
            Self::Comets => COMETS_SHORTLIST,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseEntry {
    pub body_index: usize,
    pub body_id: String,
    pub name: String,
    pub category: Category,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseColumn {
    pub kind: BrowseColumnKind,
    pub title: &'static str,
    pub count_label: String,
    pub shortlist: Vec<BrowseEntry>,
    pub all: Vec<BrowseEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseModel {
    pub counts: BrowseCounts,
    pub columns: [BrowseColumn; 3],
}

impl BrowseModel {
    pub fn from_catalog(catalog: &Catalog) -> Self {
        let counts = BrowseCounts::from_catalog(catalog);
        let columns = BrowseColumnKind::ALL.map(|kind| {
            let all = catalog
                .bodies
                .iter()
                .enumerate()
                .filter(|(_index, body)| kind.includes(body.category))
                .map(|(body_index, body)| BrowseEntry {
                    body_index,
                    body_id: body.id.clone(),
                    name: body.name.clone(),
                    category: body.category,
                })
                .collect::<Vec<_>>();
            let shortlist = kind
                .shortlist_ids()
                .iter()
                .filter_map(|id| all.iter().find(|entry| entry.body_id == *id).cloned())
                .collect();
            BrowseColumn {
                kind,
                title: kind.title(),
                count_label: count_label(kind, counts),
                shortlist,
                all,
            }
        });
        Self { counts, columns }
    }
}

fn count_label(kind: BrowseColumnKind, counts: BrowseCounts) -> String {
    match kind {
        BrowseColumnKind::PlanetsAndMoons => format!(
            "{} STAR · {} PLANETS · {} MOONS",
            counts.stars, counts.planets, counts.moons
        ),
        BrowseColumnKind::DwarfsAndAsteroids => format!(
            "{} DWARF PLANETS · {} ASTEROIDS",
            counts.dwarf_planets, counts.asteroids
        ),
        BrowseColumnKind::Comets => format!("{} COMETS", counts.comets),
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct SearchDropdownRoot;

#[derive(Component, Debug, Clone, Copy)]
struct SearchResultAction(usize);

#[derive(Component, Debug, Clone, Copy)]
pub struct BrowseMenuRoot;

#[derive(Component, Debug, Clone, Copy)]
enum BrowseAction {
    Close,
    ToggleExpanded(usize),
    TravelTo(usize),
}

struct BrowseButtonSpec<'a> {
    label: &'a str,
    accessible_label: &'a str,
    action: BrowseAction,
    tab_index: i32,
}

#[derive(Component, Debug, Clone, Copy)]
struct BrowseScrollColumn(u8);

#[derive(Resource, Debug, Default)]
struct SearchUiState {
    query: String,
    restore_query: String,
    hits: Vec<SearchHit>,
    active_input: Option<Entity>,
    pending_value: Option<String>,
    dropdown_root: Option<Entity>,
    dirty: bool,
}

impl SearchUiState {
    fn set_query(&mut self, catalog: &Catalog, query: String) {
        self.hits = search_catalog(catalog, &query);
        self.query = query;
        self.dirty = true;
    }

    fn begin_edit(&mut self, entity: Entity, current: &str) {
        self.active_input = Some(entity);
        self.restore_query = current.to_string();
        self.dirty = true;
    }

    fn end_edit(&mut self) {
        self.active_input = None;
        self.dirty = true;
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct BrowseUiState {
    open: bool,
    expanded: [bool; 3],
    root: Option<Entity>,
    dirty: bool,
    scroll_y: [f32; 3],
    restore_focus: bool,
}

impl BrowseUiState {
    pub(crate) const fn is_open(&self) -> bool {
        self.open
    }
}

pub(crate) fn consume_search_command(command: &SimCommand, state: &mut BrowseUiState) {
    match command {
        SimCommand::SetBrowseOpen(open) => {
            state.restore_focus = state.open && !open;
            state.open = *open;
            state.dirty = true;
        }
        SimCommand::SetBrowseColumnExpanded { column, expanded } => {
            if let Some(value) = state.expanded.get_mut(usize::from(*column)) {
                *value = *expanded;
                state.scroll_y[usize::from(*column)] = 0.0;
                state.dirty = true;
            }
        }
        SimCommand::RestorePresentationDefaults => {
            let was_open = state.open;
            state.open = false;
            state.expanded = [false; 3];
            state.scroll_y = [0.0; 3];
            state.restore_focus = was_open;
            state.dirty = true;
        }
        _ => {}
    }
}

pub struct SearchPlugin;

impl Plugin for SearchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SearchUiState>()
            .init_resource::<BrowseUiState>()
            .add_systems(
                Update,
                (attach_search_observers, sync_search_input)
                    .chain()
                    .in_set(SimulationSet::Input),
            )
            .add_systems(
                Update,
                (rebuild_search_dropdown, rebuild_browse_menu)
                    .chain()
                    .in_set(SimulationSet::Render),
            );
    }
}

#[allow(clippy::type_complexity)]
fn attach_search_observers(
    mut commands: Commands,
    search_inputs: Query<Entity, Added<SearchInput>>,
    menu_buttons: Query<Entity, Added<MenuBrowseButton>>,
    search_results: Query<Entity, Added<SearchResultAction>>,
    browse_actions: Query<Entity, Added<BrowseAction>>,
) {
    for entity in &search_inputs {
        commands.entity(entity).observe(handle_search_key);
    }
    for entity in &menu_buttons {
        commands.entity(entity).observe(open_browse_menu);
    }
    for entity in &search_results {
        commands.entity(entity).observe(activate_search_result);
    }
    for entity in &browse_actions {
        commands.entity(entity).observe(activate_browse_action);
    }
}

fn sync_search_input(
    focus: Res<InputFocus>,
    loaded: Option<Res<LoadedCatalog>>,
    mut state: ResMut<SearchUiState>,
    inputs: Query<(Entity, &EditableText), With<SearchInput>>,
    mut hints: Query<&mut Visibility, With<SearchHint>>,
) {
    let Ok((entity, editable)) = inputs.single() else {
        return;
    };
    let value = editable.value().to_string();
    for mut visibility in &mut hints {
        *visibility = if value.is_empty() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    let is_focused = focus.get() == Some(entity);
    if is_focused && state.active_input != Some(entity) {
        state.begin_edit(entity, &value);
    } else if !is_focused && state.active_input == Some(entity) {
        state.end_edit();
    }

    if let Some(expected) = state.pending_value.as_deref() {
        if value == expected {
            state.pending_value = None;
        } else {
            return;
        }
    }
    if value != state.query {
        if let Some(loaded) = loaded {
            state.set_query(&loaded.catalog, value);
        } else {
            state.query = value;
            state.hits.clear();
            state.dirty = true;
        }
    }
}

fn handle_search_key(
    mut input: On<FocusedInput<KeyboardInput>>,
    loaded: Option<Res<LoadedCatalog>>,
    mut focus: ResMut<InputFocus>,
    mut state: ResMut<SearchUiState>,
    mut fields: Query<&mut EditableText, With<SearchInput>>,
    mut sim_commands: ResMut<SimCommandQueue>,
) {
    if input.input.state != ButtonState::Pressed {
        return;
    }
    match input.input.key_code {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            let Some(hit) = state.hits.first().cloned() else {
                return;
            };
            sim_commands.push(SimCommand::TravelToBody(hit.body_id));
            if let Ok(mut editable) = fields.get_mut(input.focused_entity) {
                replace_editable_text(&mut editable, &hit.display_name);
            }
            if let Some(loaded) = loaded {
                state.set_query(&loaded.catalog, hit.display_name.clone());
            }
            state.restore_query = hit.display_name.clone();
            state.pending_value = Some(hit.display_name);
            state.end_edit();
            focus.clear();
            input.propagate(false);
        }
        KeyCode::Escape => {
            let restored = state.restore_query.clone();
            if let Ok(mut editable) = fields.get_mut(input.focused_entity) {
                replace_editable_text(&mut editable, &restored);
            }
            if let Some(loaded) = loaded {
                state.set_query(&loaded.catalog, restored.clone());
            }
            state.pending_value = Some(restored);
            state.end_edit();
            focus.clear();
            input.propagate(false);
        }
        _ => {}
    }
}

fn replace_editable_text(editable: &mut EditableText, replacement: &str) {
    if editable.value() != replacement {
        editable.queue_edit(TextEdit::SelectAll);
        editable.queue_edit(TextEdit::Insert(replacement.into()));
    }
}

fn open_browse_menu(
    _activate: On<Activate>,
    mut focus: ResMut<InputFocus>,
    mut sim_commands: ResMut<SimCommandQueue>,
) {
    sim_commands.push(SimCommand::SetBrowseOpen(true));
    focus.clear();
}

fn activate_search_result(
    activate: On<Activate>,
    actions: Query<&SearchResultAction>,
    loaded: Res<LoadedCatalog>,
    mut focus: ResMut<InputFocus>,
    mut state: ResMut<SearchUiState>,
    mut fields: Query<&mut EditableText, With<SearchInput>>,
    mut sim_commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    let Some(body) = loaded.catalog.bodies.get(action.0) else {
        return;
    };
    sim_commands.push(SimCommand::TravelToBody(body.id.clone()));
    if let Ok(mut editable) = fields.single_mut() {
        replace_editable_text(&mut editable, &body.name);
    }
    state.set_query(&loaded.catalog, body.name.clone());
    state.restore_query = body.name.clone();
    state.pending_value = Some(body.name.clone());
    state.end_edit();
    focus.clear();
}

fn activate_browse_action(
    activate: On<Activate>,
    actions: Query<&BrowseAction>,
    loaded: Res<LoadedCatalog>,
    browse: Res<BrowseUiState>,
    mut sim_commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    match *action {
        BrowseAction::Close => sim_commands.push(SimCommand::SetBrowseOpen(false)),
        BrowseAction::ToggleExpanded(column) => {
            if let Some(expanded) = browse.expanded.get(column) {
                sim_commands.push(SimCommand::SetBrowseColumnExpanded {
                    column: column as u8,
                    expanded: !*expanded,
                });
            }
        }
        BrowseAction::TravelTo(body_index) => {
            if let Some(body) = loaded.catalog.bodies.get(body_index) {
                sim_commands.push(SimCommand::TravelToBody(body.id.clone()));
                sim_commands.push(SimCommand::SetBrowseOpen(false));
            }
        }
    }
}

fn rebuild_search_dropdown(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    loaded: Option<Res<LoadedCatalog>>,
    mut state: ResMut<SearchUiState>,
) {
    if !state.dirty {
        return;
    }
    if let Some(root) = state.dropdown_root.take() {
        commands.entity(root).despawn();
    }
    if state.active_input.is_none() || state.query.trim().is_empty() || loaded.is_none() {
        state.dirty = false;
        return;
    }

    let root = commands
        .spawn((
            Name::new("Search results dropdown"),
            SearchDropdownRoot,
            HudSurface,
            AccessibleLabel::new(format!("Search results, {} matches", state.hits.len())),
            Node {
                position_type: PositionType::Absolute,
                top: px(TOP_BAR_HEIGHT_PX + theme.spacing.xs_px),
                right: px(theme.spacing.lg_px),
                width: px(SEARCH_DROPDOWN_WIDTH_PX),
                padding: UiRect::all(px(theme.spacing.sm_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.xs_px),
                ..default()
            },
            BackgroundColor(theme.colors.panel_elevated.color()),
            BorderColor::all(theme.colors.separator.color()),
            GlobalZIndex(SEARCH_DROPDOWN_Z_INDEX),
        ))
        .id();
    let font = asset_server.load(INTER_FONT_ASSET);
    if state.hits.is_empty() {
        spawn_text(
            &mut commands,
            root,
            &font,
            "No matching bodies",
            theme.type_scale.body_px,
            theme.colors.text_muted.color(),
            false,
        );
    } else {
        for (row, hit) in state.hits.iter().take(MAX_DROPDOWN_RESULTS).enumerate() {
            let body_category = loaded
                .as_ref()
                .and_then(|loaded| loaded.catalog.bodies.get(hit.body_index))
                .map_or("Body".to_string(), |body| body.category.to_string());
            let accessible_label = format!("Travel to {} ({body_category})", hit.display_name);
            let button = commands
                .spawn((
                    bevy::ui_widgets::Button,
                    SearchResultAction(hit.body_index),
                    AccessibleLabel::new(accessible_label),
                    TabIndex(10 + row as i32),
                    Node {
                        width: percent(100),
                        min_height: px(42),
                        padding: UiRect::horizontal(px(theme.spacing.sm_px)),
                        border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                        align_items: AlignItems::Center,
                        column_gap: px(theme.spacing.sm_px),
                        ..default()
                    },
                    BackgroundColor(if row == 0 {
                        theme.colors.panel.color()
                    } else {
                        Color::NONE
                    }),
                    ChildOf(root),
                ))
                .id();
            spawn_text(
                &mut commands,
                button,
                &font,
                &hit.display_name,
                theme.type_scale.body_px,
                theme.colors.text_primary.color(),
                false,
            );
            let detail = if hit.matched_key != hit.display_name {
                format!("{} · {}", hit.matched_key, body_category)
            } else {
                body_category
            };
            let detail_entity = spawn_text(
                &mut commands,
                button,
                &font,
                &detail,
                theme.type_scale.caption_px,
                theme.colors.text_muted.color(),
                false,
            );
            commands.entity(detail_entity).insert(Node {
                margin: UiRect::left(auto()),
                ..default()
            });
        }
    }
    state.dropdown_root = Some(root);
    state.dirty = false;
}

fn rebuild_browse_menu(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    loaded: Option<Res<LoadedCatalog>>,
    search_inputs: Query<Entity, With<SearchInput>>,
    mut focus: ResMut<InputFocus>,
    mut state: ResMut<BrowseUiState>,
) {
    if !state.dirty {
        return;
    }
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }
    let (true, Some(loaded)) = (state.open, loaded) else {
        if state.restore_focus {
            if let Ok(search_input) = search_inputs.single() {
                focus.set(search_input, FocusCause::Navigated);
            }
            state.restore_focus = false;
        }
        state.dirty = false;
        return;
    };
    let model = BrowseModel::from_catalog(&loaded.catalog);
    let font = asset_server.load(INTER_FONT_ASSET);
    let root = spawn_browse_root(&mut commands, *theme);
    let header = commands
        .spawn((
            Node {
                width: percent(100),
                height: px(44),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    spawn_text(
        &mut commands,
        header,
        &font,
        "BROWSE THE SOLAR SYSTEM",
        theme.type_scale.product_px,
        theme.colors.text_primary.color(),
        true,
    );
    let close = spawn_button(
        &mut commands,
        header,
        *theme,
        &font,
        BrowseButtonSpec {
            label: "CLOSE  ×",
            accessible_label: "Close body browse menu",
            action: BrowseAction::Close,
            tab_index: 20,
        },
    );
    commands.entity(close).insert(Node {
        margin: UiRect::left(auto()),
        min_width: px(94),
        height: px(34),
        padding: UiRect::horizontal(px(theme.spacing.md_px)),
        border: UiRect::all(px(theme.spacing.hairline_px)),
        border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    });

    let columns = commands
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                column_gap: px(theme.spacing.lg_px),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    for column in &model.columns {
        spawn_browse_column(
            &mut commands,
            columns,
            *theme,
            &font,
            column,
            state.expanded[column.kind.index()],
            state.scroll_y[column.kind.index()],
        );
    }
    state.root = Some(root);
    state.dirty = false;
}

fn spawn_browse_root(commands: &mut Commands, theme: UiTheme) -> Entity {
    commands
        .spawn((
            Name::new("Full-screen body browse menu"),
            BrowseMenuRoot,
            HudSurface,
            AccessibleLabel::new("Browse Solar System bodies by category"),
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                top: px(0),
                right: px(0),
                bottom: px(0),
                left: px(0),
                width: percent(100),
                height: percent(100),
                padding: UiRect::all(px(theme.spacing.xl_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.lg_px),
                ..default()
            },
            BackgroundColor(theme.colors.scrim.color()),
            GlobalZIndex(MENU_Z_INDEX),
        ))
        .id()
}

fn spawn_browse_column(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    column: &BrowseColumn,
    expanded: bool,
    scroll_y: f32,
) {
    let column_root = commands
        .spawn((
            AccessibleLabel::new(format!("{} browse column", column.title)),
            Node {
                width: percent(33),
                flex_grow: 1.0,
                min_width: px(0),
                padding: UiRect::all(px(theme.spacing.md_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme.colors.panel.color()),
            BorderColor::all(theme.colors.separator.color()),
            ChildOf(parent),
        ))
        .id();
    spawn_text(
        commands,
        column_root,
        font,
        column.title,
        theme.type_scale.title_px,
        theme.colors.text_primary.color(),
        true,
    );
    spawn_text(
        commands,
        column_root,
        font,
        &column.count_label,
        theme.type_scale.caption_px,
        theme.colors.text_muted.color(),
        true,
    );
    commands.spawn((
        Node {
            width: percent(100),
            height: px(theme.spacing.hairline_px),
            ..default()
        },
        BackgroundColor(theme.colors.separator.color()),
        ChildOf(column_root),
    ));
    let scroll_position = ScrollPosition(Vec2::new(0.0, scroll_y));
    let list = commands
        .spawn((
            BrowseScrollColumn(column.kind.index() as u8),
            UiScrollSurface,
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.xs_px),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            scroll_position,
            ChildOf(column_root),
        ))
        .observe(scroll_browse_column)
        .id();
    let entries = if expanded {
        &column.all
    } else {
        &column.shortlist
    };
    for (row, entry) in entries.iter().enumerate() {
        spawn_button(
            commands,
            list,
            theme,
            font,
            BrowseButtonSpec {
                label: &format!("{}  →", entry.name),
                accessible_label: &format!("Select and travel to {}", entry.name),
                action: BrowseAction::TravelTo(entry.body_index),
                tab_index: 30 + (column.kind.index() * 100 + row) as i32,
            },
        );
    }
    if column.shortlist.len() < column.all.len() {
        spawn_button(
            commands,
            column_root,
            theme,
            font,
            BrowseButtonSpec {
                label: if expanded {
                    "SHOW CURATED LIST"
                } else {
                    "SHOW ALL"
                },
                accessible_label: if expanded {
                    "Collapse to curated shortlist"
                } else {
                    "Expand the complete category list"
                },
                action: BrowseAction::ToggleExpanded(column.kind.index()),
                tab_index: 390 + column.kind.index() as i32,
            },
        );
    }
}

fn scroll_browse_column(
    mut scroll: On<Pointer<Scroll>>,
    mut columns: Query<(&BrowseScrollColumn, &mut ScrollPosition, &ComputedNode)>,
    mut state: ResMut<BrowseUiState>,
) {
    let Ok((column, mut position, node)) = columns.get_mut(scroll.entity) else {
        return;
    };
    position.y = next_browse_scroll_y(
        position.y,
        scroll.y,
        scroll.unit,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor,
    );
    state.scroll_y[usize::from(column.0)] = position.y;
    scroll.propagate(false);
}

fn next_browse_scroll_y(
    current: f32,
    input_y: f32,
    unit: MouseScrollUnit,
    content_height: f32,
    visible_height: f32,
    inverse_scale_factor: f32,
) -> f32 {
    let delta_y = match unit {
        MouseScrollUnit::Line => input_y * 28.0,
        MouseScrollUnit::Pixel => input_y,
    };
    let range = (content_height - visible_height).max(0.0) * inverse_scale_factor;
    (current - delta_y).clamp(0.0, range)
}

fn spawn_button(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    spec: BrowseButtonSpec<'_>,
) -> Entity {
    let button = commands
        .spawn((
            bevy::ui_widgets::Button,
            spec.action,
            AccessibleLabel::new(spec.accessible_label.to_string()),
            TabIndex(spec.tab_index),
            Node {
                width: percent(100),
                min_height: px(32),
                padding: UiRect::horizontal(px(theme.spacing.sm_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.colors.panel_elevated.color()),
            ChildOf(parent),
        ))
        .id();
    spawn_text(
        commands,
        button,
        font,
        spec.label,
        theme.type_scale.body_px,
        theme.colors.text_primary.color(),
        false,
    );
    button
}

fn spawn_text(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    value: &str,
    font_size: f32,
    color: Color,
    tracked: bool,
) -> Entity {
    let mut entity = commands.spawn((
        Text::new(value),
        TextFont {
            font: font.clone().into(),
            font_size: font_size.into(),
            ..default()
        },
        TextColor(color),
        TextLayout {
            linebreak: LineBreak::NoWrap,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(parent),
    ));
    if tracked {
        entity.insert(LetterSpacing::Px(
            UiTheme::DARK.type_scale.uppercase_tracking_px,
        ));
    }
    entity.id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_catalog_text;
    use bevy::{
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        input::{keyboard::Key, InputPlugin},
        input_focus::{InputDispatchPlugin, InputFocusPlugin},
        text::Font,
        window::PrimaryWindow,
    };
    use std::cmp::Ordering;

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn real_catalog() -> Catalog {
        load_catalog_text(REAL_CATALOG).expect("committed catalog must load")
    }

    #[test]
    fn every_exact_catalog_search_key_is_rank_one() {
        let catalog = real_catalog();
        let mut checked = 0;
        for (body_index, body) in catalog.bodies.iter().enumerate() {
            let keys = std::iter::once(body.name.as_str())
                .chain(body.designation.as_deref())
                .chain(body.aliases.iter().map(String::as_str));
            for key in keys {
                let hits = search_catalog(&catalog, &key.to_uppercase());
                assert_eq!(
                    hits.first().map(|hit| hit.body_index),
                    Some(body_index),
                    "exact key {key:?} did not rank {} first",
                    body.name
                );
                assert_eq!(hits[0].kind, SearchMatchKind::Exact, "key {key:?}");
                checked += 1;
            }
        }
        assert!(checked > catalog.bodies.len());
    }

    #[test]
    fn fuzzy_candidates_never_shadow_exact_atlas_keys() {
        let catalog = real_catalog();
        let atlas = catalog.find("3I/ATLAS").unwrap();
        let atlas_id = atlas.id.clone();
        for query in ["3I/ATLAS", "C/2025 N1"] {
            let hits = search_catalog(&catalog, query);
            assert_eq!(hits[0].body_id, atlas_id);
            assert_eq!(hits[0].kind, SearchMatchKind::Exact);
            assert_eq!(hits.iter().filter(|hit| hit.body_id == atlas_id).count(), 1);
        }

        let hale = search_catalog(&catalog, "hale");
        assert_eq!(
            hale.first().map(|hit| hit.body_id.as_str()),
            Some("hale_bopp")
        );
        assert_eq!(hale[0].kind, SearchMatchKind::Prefix);

        let typo = search_catalog(&catalog, "jupter");
        assert_eq!(
            typo.first().map(|hit| hit.body_id.as_str()),
            Some("jupiter")
        );
        assert_eq!(typo[0].kind, SearchMatchKind::Fuzzy);
    }

    #[test]
    fn browse_counts_and_expandable_lists_come_from_the_catalog() {
        let catalog = real_catalog();
        let model = BrowseModel::from_catalog(&catalog);
        assert_eq!(
            model.counts,
            BrowseCounts {
                stars: 1,
                planets: 8,
                dwarf_planets: 9,
                asteroids: 8,
                moons: 32,
                comets: 8,
            }
        );
        assert_eq!(model.columns[0].all.len(), 41);
        assert_eq!(model.columns[1].all.len(), 17);
        assert_eq!(model.columns[2].all.len(), 8);
        for column in &model.columns {
            assert!(!column.shortlist.is_empty());
            assert!(column.shortlist.len() < column.all.len());
            assert!(column
                .shortlist
                .iter()
                .all(|entry| column.all.contains(entry)));
            assert_eq!(column.shortlist.len(), column.kind.shortlist_ids().len());
        }
    }

    #[test]
    fn browse_column_scroll_clamps_line_and_pixel_input() {
        assert_eq!(
            next_browse_scroll_y(100.0, -2.0, MouseScrollUnit::Line, 1_000.0, 600.0, 1.0),
            156.0
        );
        assert_eq!(
            next_browse_scroll_y(390.0, -50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0,),
            400.0
        );
        assert_eq!(
            next_browse_scroll_y(10.0, 50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0),
            0.0
        );
    }

    #[test]
    fn browse_menu_renders_curated_then_complete_accessible_entry_sets() {
        let catalog = real_catalog();
        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default()))
            .init_asset::<Font>()
            .insert_resource(LoadedCatalog::new(catalog))
            .insert_resource(UiTheme::default())
            .insert_resource(BrowseUiState {
                open: true,
                dirty: true,
                ..default()
            })
            .init_resource::<InputFocus>()
            .add_systems(Update, rebuild_browse_menu);
        app.update();

        let world = app.world_mut();
        assert_eq!(world.query::<&BrowseMenuRoot>().iter(world).count(), 1);
        let curated_entries = world
            .query::<(&BrowseAction, &AccessibleLabel)>()
            .iter(world)
            .filter(|(action, label)| {
                matches!(action, BrowseAction::TravelTo(_)) && !label.0.trim().is_empty()
            })
            .count();
        assert_eq!(
            curated_entries,
            PLANETS_AND_MOONS_SHORTLIST.len()
                + DWARFS_AND_ASTEROIDS_SHORTLIST.len()
                + COMETS_SHORTLIST.len()
        );

        {
            let mut state = world.resource_mut::<BrowseUiState>();
            state.expanded = [true; 3];
            state.dirty = true;
        }
        app.update();
        let world = app.world_mut();
        let all_entries = world
            .query::<&BrowseAction>()
            .iter(world)
            .filter(|action| matches!(action, BrowseAction::TravelTo(_)))
            .count();
        assert_eq!(all_entries, 66);
        assert_eq!(
            world
                .query::<&BrowseAction>()
                .iter(world)
                .filter(|action| matches!(action, BrowseAction::ToggleExpanded(_)))
                .count(),
            3
        );

        let search_input = world.spawn(SearchInput).id();
        consume_search_command(
            &SimCommand::SetBrowseOpen(false),
            &mut world.resource_mut::<BrowseUiState>(),
        );
        app.update();
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(search_input)
        );
    }

    #[test]
    fn edit_model_restores_on_escape_and_enter_targets_the_top_hit() {
        let catalog = real_catalog();
        let entity = Entity::from_bits(42);
        let mut state = SearchUiState::default();
        state.begin_edit(entity, "Earth");
        state.set_query(&catalog, "hale".to_string());
        assert_eq!(
            state.hits.first().map(|hit| hit.body_id.as_str()),
            Some("hale_bopp")
        );
        assert_eq!(state.restore_query, "Earth");

        let enter_target = state.hits.first().map(|hit| hit.body_id.clone());
        assert_eq!(enter_target.as_deref(), Some("hale_bopp"));
        state.set_query(&catalog, state.restore_query.clone());
        assert_eq!(state.query, "Earth");
        assert_eq!(
            state.hits.first().map(|hit| hit.body_id.as_str()),
            Some("earth")
        );
    }

    #[test]
    fn focused_search_enter_enqueues_the_top_ranked_travel_command() {
        let catalog = real_catalog();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            InputPlugin,
            InputFocusPlugin,
            InputDispatchPlugin,
        ))
        .insert_resource(LoadedCatalog::new(catalog))
        .init_resource::<SearchUiState>()
        .init_resource::<SimCommandQueue>()
        .add_systems(Update, (attach_search_observers, sync_search_input).chain());
        let window = app
            .world_mut()
            .spawn((Window::default(), PrimaryWindow))
            .id();
        let input = app
            .world_mut()
            .spawn((SearchInput, EditableText::new("hale")))
            .id();

        app.update();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(input, FocusCause::Navigated);
        app.update();
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Enter,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        app.update();

        let queued: Vec<_> = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .collect();
        assert_eq!(queued, vec![SimCommand::TravelToBody("hale_bopp".into())]);
        assert_eq!(app.world().resource::<InputFocus>().get(), None);
    }

    #[test]
    fn ranking_is_deterministic_for_tied_short_queries() {
        let catalog = real_catalog();
        let first = search_catalog(&catalog, "ma");
        let second = search_catalog(&catalog, "MA");
        assert_eq!(first, second);
        assert!(first
            .windows(2)
            .all(|pair| pair[0].sort_key().cmp(&pair[1].sort_key()) != Ordering::Greater));
    }
}
