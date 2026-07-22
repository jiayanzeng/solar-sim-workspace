//! WP10 contextual body panel and render-only view options — Rev C §§4.1 and 9.2.
//!
//! The view model is derived exclusively from the validated catalog. UI state
//! is local and snapshot-ready for WP14; body-size exaggeration is consumed by
//! UIP-2's render-only apparent-size system, while travel continues through
//! `SimCommand`.

use crate::control::{CameraController, SimCommand, SimCommandQueue};
use crate::input_intent::UiScrollSurface;
use crate::layers::{HudSurface, LayerState};
use crate::ui_kit::{
    section_header, NavigationDestination, NavigationStack, UiTheme, WidgetSpec, WidgetVisualState,
    INTER_FONT_ASSET, TOP_BAR_HEIGHT_PX,
};
use crate::{
    format_distance_km, AppSettings, BodyVisual, DistanceUnit, LoadedCatalog, SimulationSet,
    KM_PER_RENDER_UNIT, TIME_BAR_HEIGHT_PX,
};
use bevy::{
    input::mouse::MouseScrollUnit,
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, InputFocus,
    },
    prelude::*,
    text::{Font, LetterSpacing, LineBreak, TextLayout},
    ui_widgets::Activate,
};
use sim_core::catalog::{BodyRecord, Catalog, Category};
use sim_core::time::JULIAN_YEAR_S;
use std::collections::BTreeMap;
use std::fmt;

const PANEL_WIDTH_PX: f32 = 340.0;
const PANEL_COLLAPSED_SIZE_PX: f32 = 44.0;
const PANEL_MARGIN_PX: f32 = 16.0;
const PANEL_Z_INDEX: i32 = 85;
const PANEL_TAB_GROUP_ORDER: i32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeftPanelTab {
    Info,
    Collection,
    ViewOptions,
}

impl LeftPanelTab {
    fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Collection => "MOONS",
            Self::ViewOptions => "VIEW",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyLinkViewModel {
    pub body_index: usize,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrbitalPeriodViewModel {
    NotApplicable,
    Hyperbolic,
    Elliptic { seconds: f64, label: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptionViewModel {
    Curated(String),
    CatalogLint(String),
}

impl DescriptionViewModel {
    pub fn text(&self) -> &str {
        match self {
            Self::Curated(text) | Self::CatalogLint(text) => text,
        }
    }

    pub fn is_catalog_lint(&self) -> bool {
        matches!(self, Self::CatalogLint(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoonCollectionViewModel {
    pub parent: BodyLinkViewModel,
    pub label: String,
    pub children: Vec<BodyLinkViewModel>,
}

impl MoonCollectionViewModel {
    pub fn count(&self) -> usize {
        self.children.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyInfoViewModel {
    pub body: BodyLinkViewModel,
    pub category: Category,
    pub category_label: String,
    pub category_color_srgb: (u8, u8, u8),
    pub radius_km: f64,
    pub radius_label: String,
    pub orbital_period: OrbitalPeriodViewModel,
    pub parent: Option<BodyLinkViewModel>,
    pub description: DescriptionViewModel,
    pub collection: Option<MoonCollectionViewModel>,
    pub tabs: Vec<LeftPanelTab>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoViewModelError {
    MissingBody { index: usize },
    EmptyName { id: String },
    MissingParent { id: String },
    UnknownParent { id: String, parent: String },
    MissingParentGm { id: String, parent: String },
    MissingOrbit { id: String },
    InvalidPeriod { id: String },
    MissingDescriptionLint { id: String },
}

impl fmt::Display for InfoViewModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBody { index } => write!(f, "catalog has no body at index {index}"),
            Self::EmptyName { id } => write!(f, "'{id}' has an empty display name"),
            Self::MissingParent { id } => write!(f, "'{id}' has no parent"),
            Self::UnknownParent { id, parent } => {
                write!(f, "'{id}' references unknown parent '{parent}'")
            }
            Self::MissingParentGm { id, parent } => {
                write!(
                    f,
                    "'{id}' cannot derive a period because '{parent}' has no GM"
                )
            }
            Self::MissingOrbit { id } => write!(f, "'{id}' has no orbit"),
            Self::InvalidPeriod { id } => write!(f, "'{id}' derived an invalid orbital period"),
            Self::MissingDescriptionLint { id } => {
                write!(f, "'{id}' has no description and WP3 emitted no lint")
            }
        }
    }
}

pub fn body_info_view_model(
    catalog: &Catalog,
    body_index: usize,
) -> Result<BodyInfoViewModel, InfoViewModelError> {
    body_info_view_model_with_units(catalog, body_index, DistanceUnit::Kilometers)
}

pub fn body_info_view_model_with_units(
    catalog: &Catalog,
    body_index: usize,
    units: DistanceUnit,
) -> Result<BodyInfoViewModel, InfoViewModelError> {
    let body = catalog
        .bodies
        .get(body_index)
        .ok_or(InfoViewModelError::MissingBody { index: body_index })?;
    if body.name.trim().is_empty() {
        return Err(InfoViewModelError::EmptyName {
            id: body.id.clone(),
        });
    }

    let id_index = catalog.id_index();
    let parent = match body.parent.as_deref() {
        Some(parent_id) => {
            let parent_index = id_index.get(parent_id).copied().ok_or_else(|| {
                InfoViewModelError::UnknownParent {
                    id: body.id.clone(),
                    parent: parent_id.to_string(),
                }
            })?;
            Some(body_link(catalog, parent_index)?)
        }
        None if body.category == Category::Star => None,
        None => {
            return Err(InfoViewModelError::MissingParent {
                id: body.id.clone(),
            });
        }
    };

    let orbital_period = if body.category == Category::Star {
        OrbitalPeriodViewModel::NotApplicable
    } else {
        let orbit = body
            .orbit
            .as_ref()
            .ok_or_else(|| InfoViewModelError::MissingOrbit {
                id: body.id.clone(),
            })?;
        if orbit.elements.is_hyperbolic() {
            OrbitalPeriodViewModel::Hyperbolic
        } else {
            let parent_id =
                body.parent
                    .as_deref()
                    .ok_or_else(|| InfoViewModelError::MissingParent {
                        id: body.id.clone(),
                    })?;
            let parent_index = id_index.get(parent_id).copied().ok_or_else(|| {
                InfoViewModelError::UnknownParent {
                    id: body.id.clone(),
                    parent: parent_id.to_string(),
                }
            })?;
            let parent_gm = catalog.bodies[parent_index].gm_km3_s2.ok_or_else(|| {
                InfoViewModelError::MissingParentGm {
                    id: body.id.clone(),
                    parent: parent_id.to_string(),
                }
            })?;
            let seconds = orbit
                .period_s(parent_gm)
                .filter(|period| period.is_finite() && *period > 0.0)
                .ok_or_else(|| InfoViewModelError::InvalidPeriod {
                    id: body.id.clone(),
                })?;
            OrbitalPeriodViewModel::Elliptic {
                seconds,
                label: format_period(seconds),
            }
        }
    };

    let description = if body.description.trim().is_empty() {
        let prefix = format!("'{}': description is empty", body.id);
        let lint = catalog
            .lint()
            .into_iter()
            .find(|lint| lint.starts_with(&prefix))
            .ok_or_else(|| InfoViewModelError::MissingDescriptionLint {
                id: body.id.clone(),
            })?;
        DescriptionViewModel::CatalogLint(lint)
    } else {
        DescriptionViewModel::Curated(body.description.trim().to_string())
    };

    let collection = moon_collection_for_parent(catalog, body_index)?;
    let mut tabs = vec![LeftPanelTab::Info];
    if collection.is_some() {
        tabs.push(LeftPanelTab::Collection);
    }
    tabs.push(LeftPanelTab::ViewOptions);

    Ok(BodyInfoViewModel {
        body: body_link(catalog, body_index)?,
        category: body.category,
        category_label: body.category.to_string(),
        category_color_srgb: body.color_srgb,
        radius_km: body.radius_km,
        radius_label: format_distance_km(body.radius_km, units),
        orbital_period,
        parent,
        description,
        collection,
        tabs,
    })
}

pub fn moon_collections(
    catalog: &Catalog,
) -> Result<Vec<MoonCollectionViewModel>, InfoViewModelError> {
    let mut collections = Vec::new();
    for parent_index in 0..catalog.bodies.len() {
        if let Some(collection) = moon_collection_for_parent(catalog, parent_index)? {
            collections.push(collection);
        }
    }
    Ok(collections)
}

fn moon_collection_for_parent(
    catalog: &Catalog,
    parent_index: usize,
) -> Result<Option<MoonCollectionViewModel>, InfoViewModelError> {
    let parent = catalog
        .bodies
        .get(parent_index)
        .ok_or(InfoViewModelError::MissingBody {
            index: parent_index,
        })?;
    let children: Vec<_> = catalog
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, body)| {
            body.category == Category::Moon && body.parent.as_deref() == Some(parent.id.as_str())
        })
        .map(|(index, _)| body_link(catalog, index))
        .collect::<Result<_, _>>()?;
    if children.is_empty() {
        return Ok(None);
    }
    Ok(Some(MoonCollectionViewModel {
        parent: body_link(catalog, parent_index)?,
        label: format!("Moons of {} ({})", parent.name, children.len()),
        children,
    }))
}

fn body_link(
    catalog: &Catalog,
    body_index: usize,
) -> Result<BodyLinkViewModel, InfoViewModelError> {
    let body = catalog
        .bodies
        .get(body_index)
        .ok_or(InfoViewModelError::MissingBody { index: body_index })?;
    if body.name.trim().is_empty() {
        return Err(InfoViewModelError::EmptyName {
            id: body.id.clone(),
        });
    }
    Ok(BodyLinkViewModel {
        body_index,
        id: body.id.clone(),
        name: body.name.clone(),
    })
}

fn format_period(seconds: f64) -> String {
    let days = seconds / 86_400.0;
    if days < 2.0 {
        format!("{:.1} hours", seconds / 3_600.0)
    } else if seconds < 2.0 * JULIAN_YEAR_S {
        format!("{days:.1} days")
    } else {
        format!("{:.2} years", seconds / JULIAN_YEAR_S)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BodySizeScale {
    #[default]
    X1,
    X10,
    X50,
}

impl BodySizeScale {
    pub const ALL: [Self; 3] = [Self::X1, Self::X10, Self::X50];

    pub const fn multiplier(self) -> f32 {
        match self {
            Self::X1 => 1.0,
            Self::X10 => 10.0,
            Self::X50 => 50.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::X1 => "×1",
            Self::X10 => "×10",
            Self::X50 => "×50",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum MoonVisibilityMode {
    Major,
    #[default]
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewOptionsSnapshot {
    pub panel_collapsed: bool,
    pub body_size: BodySizeScale,
    pub moon_visibility_by_system: BTreeMap<String, MoonVisibilityMode>,
    pub local_orbit_visibility: BTreeMap<String, bool>,
}

impl Default for ViewOptionsSnapshot {
    fn default() -> Self {
        Self {
            panel_collapsed: false,
            body_size: BodySizeScale::X1,
            moon_visibility_by_system: BTreeMap::new(),
            local_orbit_visibility: BTreeMap::new(),
        }
    }
}

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewOptionsState {
    snapshot: ViewOptionsSnapshot,
}

impl ViewOptionsState {
    pub fn persistence_snapshot(&self) -> ViewOptionsSnapshot {
        self.snapshot.clone()
    }

    pub fn restore_persistence_snapshot(&mut self, snapshot: ViewOptionsSnapshot) {
        self.snapshot = snapshot;
    }

    pub fn panel_collapsed(&self) -> bool {
        self.snapshot.panel_collapsed
    }

    pub fn set_panel_collapsed(&mut self, collapsed: bool) -> bool {
        if self.snapshot.panel_collapsed == collapsed {
            return false;
        }
        self.snapshot.panel_collapsed = collapsed;
        true
    }

    pub fn body_size(&self) -> BodySizeScale {
        self.snapshot.body_size
    }

    pub fn set_body_size(&mut self, scale: BodySizeScale) -> bool {
        if self.snapshot.body_size == scale {
            return false;
        }
        self.snapshot.body_size = scale;
        true
    }

    pub fn moon_visibility(&self, system_id: &str) -> MoonVisibilityMode {
        self.snapshot
            .moon_visibility_by_system
            .get(system_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_moon_visibility(
        &mut self,
        system_id: impl Into<String>,
        mode: MoonVisibilityMode,
    ) -> bool {
        let system_id = system_id.into();
        if self.moon_visibility(&system_id) == mode {
            return false;
        }
        if mode == MoonVisibilityMode::default() {
            self.snapshot.moon_visibility_by_system.remove(&system_id);
        } else {
            self.snapshot
                .moon_visibility_by_system
                .insert(system_id, mode);
        }
        true
    }

    pub fn local_orbit_visible(&self, body_id: &str) -> bool {
        self.snapshot
            .local_orbit_visibility
            .get(body_id)
            .copied()
            .unwrap_or(true)
    }

    pub fn set_local_orbit_visible(&mut self, body_id: impl Into<String>, visible: bool) -> bool {
        let body_id = body_id.into();
        if self.local_orbit_visible(&body_id) == visible {
            return false;
        }
        if visible {
            self.snapshot.local_orbit_visibility.remove(&body_id);
        } else {
            self.snapshot
                .local_orbit_visibility
                .insert(body_id, visible);
        }
        true
    }
}

pub(crate) fn body_passes_moon_visibility(body: &BodyRecord, settings: &ViewOptionsState) -> bool {
    if body.category != Category::Moon {
        return true;
    }
    body.parent.as_deref().is_some_and(|system_id| {
        settings.moon_visibility(system_id) == MoonVisibilityMode::All || body.is_major_moon
    })
}

/// Apply the shared focus-system and per-system Major/All moon gates.
pub(crate) fn body_passes_contextual_moon_visibility(
    body_index: usize,
    focus_body_index: usize,
    loaded: &LoadedCatalog,
    settings: Option<&ViewOptionsState>,
) -> bool {
    let Some(body) = loaded.catalog.bodies.get(body_index) else {
        return false;
    };
    if body.category != Category::Moon {
        return true;
    }
    loaded.system_index_for_body(body_index) == loaded.system_index_for_body(focus_body_index)
        && settings.is_none_or(|settings| body_passes_moon_visibility(body, settings))
}

pub fn rendered_body_radius_units(radius_km: f64, scale: BodySizeScale) -> f32 {
    (radius_km / KM_PER_RENDER_UNIT) as f32 * scale.multiplier()
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LeftPanelRoot;

#[derive(Component, Debug, Clone, Copy, Default)]
struct LeftPanelContent;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ActivePanelPage {
    #[default]
    Info,
    Collection,
    ViewOptions,
}

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub(crate) struct LeftPanelUiState {
    selected_body_index: Option<usize>,
    page: ActivePanelPage,
    dirty: bool,
    scroll_y: f32,
    reset_scroll_on_rebuild: bool,
    restore_focus: Option<PanelAction>,
    rendered_view_options: Option<ViewOptionsState>,
    rendered_units: Option<DistanceUnit>,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
enum PanelAction {
    ToggleCollapsed,
    SelectPage(ActivePanelPage),
    TravelTo(usize),
    SetBodySize(BodySizeScale),
    SetMoonVisibility {
        system_index: usize,
        mode: MoonVisibilityMode,
    },
    ToggleLocalOrbit(usize),
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NavigationSyncSet;

pub struct LeftPanelPlugin;

impl Plugin for LeftPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewOptionsState>()
            .init_resource::<LeftPanelUiState>()
            .add_systems(
                Update,
                (
                    sync_left_panel_selection.in_set(NavigationSyncSet),
                    rebuild_left_panel,
                    apply_view_options,
                )
                    .chain()
                    .in_set(SimulationSet::Render),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedNavigationDestination {
    pub(crate) body_index: usize,
    pub(crate) tab: LeftPanelTab,
}

pub(crate) fn resolve_navigation_destination(
    loaded: &LoadedCatalog,
    destination: &NavigationDestination,
) -> Option<ResolvedNavigationDestination> {
    let (body_index, tab) = match destination {
        NavigationDestination::Root => (loaded.index_of("sun")?, LeftPanelTab::Info),
        NavigationDestination::Body { body_id } => (loaded.index_of(body_id)?, LeftPanelTab::Info),
        NavigationDestination::Collection { parent_id } => {
            let parent_index = loaded.index_of(parent_id)?;
            let parent = loaded.catalog.bodies.get(parent_index)?;
            let has_moons = loaded.catalog.bodies.iter().any(|body| {
                body.category == Category::Moon
                    && body.parent.as_deref() == Some(parent.id.as_str())
            });
            if !has_moons {
                return None;
            }
            (parent_index, LeftPanelTab::Collection)
        }
    };
    Some(ResolvedNavigationDestination { body_index, tab })
}

pub(crate) fn consume_left_panel_command(
    command: &SimCommand,
    loaded: Option<&LoadedCatalog>,
    settings: &mut ViewOptionsState,
    state: &mut LeftPanelUiState,
    navigation: &mut NavigationStack,
) {
    match command {
        SimCommand::SetBodySize(scale) => {
            state.dirty |= settings.set_body_size(*scale);
        }
        SimCommand::SetMoonVisibility { system_id, mode }
            if loaded.is_some_and(|loaded| loaded.index_of(system_id).is_some()) =>
        {
            state.dirty |= settings.set_moon_visibility(system_id.clone(), *mode);
        }
        SimCommand::SetLocalOrbitVisibility { body_id, visible }
            if loaded.is_some_and(|loaded| loaded.index_of(body_id).is_some()) =>
        {
            state.dirty |= settings.set_local_orbit_visible(body_id.clone(), *visible);
        }
        SimCommand::SetLeftPanelCollapsed(collapsed) => {
            state.dirty |= settings.set_panel_collapsed(*collapsed);
        }
        SimCommand::SelectBody(body_id) | SimCommand::TravelToBody(body_id) => {
            let Some(loaded) = loaded else {
                return;
            };
            let Some(body_index) = loaded.index_of(body_id) else {
                return;
            };
            apply_body_selection(loaded, body_index, state, navigation);
        }
        SimCommand::TravelToRegionPreset(_) => navigation.truncate(1),
        SimCommand::SetLeftPanelTab(tab) => {
            if *tab == LeftPanelTab::Collection {
                let (Some(loaded), Some(selected)) = (loaded, state.selected_body_index) else {
                    return;
                };
                let Some(body) = loaded.catalog.bodies.get(selected) else {
                    return;
                };
                let destination = NavigationDestination::Collection {
                    parent_id: body.id.clone(),
                };
                if resolve_navigation_destination(loaded, &destination).is_none() {
                    return;
                }
            }
            let page = match tab {
                LeftPanelTab::Info => ActivePanelPage::Info,
                LeftPanelTab::Collection => ActivePanelPage::Collection,
                LeftPanelTab::ViewOptions => ActivePanelPage::ViewOptions,
            };
            if state.page != page {
                state.page = page;
                state.dirty = true;
                state.scroll_y = 0.0;
                state.reset_scroll_on_rebuild = true;
            }
            if let (Some(loaded), Some(selected)) = (loaded, state.selected_body_index) {
                sync_navigation_to_body(loaded, selected, navigation);
                if *tab == LeftPanelTab::Collection {
                    if let Some(body) = loaded.catalog.bodies.get(selected) {
                        navigation.push_collection(body.id.clone(), "Moons");
                    }
                }
            }
        }
        SimCommand::RestorePresentationDefaults => {
            let defaults = ViewOptionsState::default();
            if *settings != defaults {
                *settings = defaults;
                state.dirty = true;
            }
        }
        SimCommand::NavigateBreadcrumb { depth, target_id } => {
            let Some(loaded) = loaded else {
                return;
            };
            let Some(destination) = navigation.destination_at(*depth, target_id).cloned() else {
                return;
            };
            let Some(resolved) = resolve_navigation_destination(loaded, &destination) else {
                return;
            };
            navigation.truncate(depth.saturating_add(1));
            let page = match resolved.tab {
                LeftPanelTab::Info => ActivePanelPage::Info,
                LeftPanelTab::Collection => ActivePanelPage::Collection,
                LeftPanelTab::ViewOptions => ActivePanelPage::ViewOptions,
            };
            if state.selected_body_index != Some(resolved.body_index) || state.page != page {
                state.selected_body_index = Some(resolved.body_index);
                state.page = page;
                state.scroll_y = 0.0;
                state.reset_scroll_on_rebuild = true;
                state.dirty = true;
            }
        }
        _ => {}
    }
}

pub(crate) fn sync_left_panel_selection_state(
    camera: &CameraController,
    loaded: &LoadedCatalog,
    state: &mut LeftPanelUiState,
    navigation: &mut NavigationStack,
) {
    let selected = camera.selected_body_index();
    if state.selected_body_index == Some(selected) {
        return;
    }
    apply_body_selection(loaded, selected, state, navigation);
}

fn apply_body_selection(
    loaded: &LoadedCatalog,
    selected: usize,
    state: &mut LeftPanelUiState,
    navigation: &mut NavigationStack,
) {
    if state.selected_body_index != Some(selected) || state.page != ActivePanelPage::Info {
        state.selected_body_index = Some(selected);
        state.page = ActivePanelPage::Info;
        state.dirty = true;
        state.scroll_y = 0.0;
        state.reset_scroll_on_rebuild = true;
    }
    sync_navigation_to_body(loaded, selected, navigation);
}

pub(crate) fn left_panel_replay_state(state: &LeftPanelUiState) -> (Option<usize>, LeftPanelTab) {
    let tab = match state.page {
        ActivePanelPage::Info => LeftPanelTab::Info,
        ActivePanelPage::Collection => LeftPanelTab::Collection,
        ActivePanelPage::ViewOptions => LeftPanelTab::ViewOptions,
    };
    (state.selected_body_index, tab)
}

fn sync_left_panel_selection(
    camera: Res<CameraController>,
    loaded: Option<Res<LoadedCatalog>>,
    mut state: ResMut<LeftPanelUiState>,
    mut navigation: ResMut<NavigationStack>,
) {
    let Some(loaded) = loaded else {
        return;
    };
    if state.selected_body_index == Some(camera.selected_body_index()) {
        return;
    }
    let navigation_before = navigation.clone();
    sync_left_panel_selection_state(
        &camera,
        &loaded,
        state.bypass_change_detection(),
        navigation.bypass_change_detection(),
    );
    state.set_changed();
    if *navigation != navigation_before {
        navigation.set_changed();
    }
}

fn sync_navigation_to_body(
    loaded: &LoadedCatalog,
    body_index: usize,
    navigation: &mut NavigationStack,
) {
    navigation.truncate(1);
    let Some(body) = loaded.catalog.bodies.get(body_index) else {
        return;
    };
    if body.category == Category::Star {
        return;
    }
    if body.category == Category::Moon {
        if let Some(parent) = body
            .parent
            .as_deref()
            .and_then(|parent_id| loaded.index_of(parent_id))
            .and_then(|parent_index| loaded.catalog.bodies.get(parent_index))
        {
            navigation.push(parent.id.clone(), parent.name.clone());
        }
    }
    navigation.push(body.id.clone(), body.name.clone());
}

#[allow(clippy::too_many_arguments)]
fn rebuild_left_panel(
    mut commands: Commands,
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    loaded: Option<Res<LoadedCatalog>>,
    view_options: Res<ViewOptionsState>,
    app_settings: Res<AppSettings>,
    mut state: ResMut<LeftPanelUiState>,
    existing_roots: Query<Entity, With<LeftPanelRoot>>,
    focus: Option<Res<InputFocus>>,
    panel_actions: Query<&PanelAction>,
    contents: Query<&ScrollPosition, With<LeftPanelContent>>,
) {
    let render_inputs_changed = state.rendered_view_options.as_ref() != Some(&*view_options)
        || state.rendered_units != Some(app_settings.units);
    if !state.dirty && !render_inputs_changed {
        return;
    }
    let (Some(loaded), Some(body_index)) = (loaded, state.selected_body_index) else {
        return;
    };
    state.rendered_view_options = Some(view_options.clone());
    state.rendered_units = Some(app_settings.units);
    if let Some(focused) = focus.as_deref().and_then(InputFocus::get) {
        if let Ok(action) = panel_actions.get(focused) {
            state.restore_focus = Some(*action);
        }
    }
    if !std::mem::take(&mut state.reset_scroll_on_rebuild) {
        if let Ok(position) = contents.single() {
            state.scroll_y = position.y;
        }
    }
    for root in &existing_roots {
        commands.entity(root).despawn();
    }

    let font = asset_server.load(INTER_FONT_ASSET);
    let root = spawn_panel_root(&mut commands, *theme, view_options.panel_collapsed());
    if view_options.panel_collapsed() {
        spawn_action_button(
            &mut commands,
            root,
            *theme,
            &font,
            "›",
            "Expand body information panel",
            PanelAction::ToggleCollapsed,
            true,
        );
        queue_panel_focus_restore(
            &mut commands,
            state.restore_focus.take(),
            PanelAction::ToggleCollapsed,
        );
        state.dirty = false;
        return;
    }

    match body_info_view_model_with_units(&loaded.catalog, body_index, app_settings.units) {
        Ok(model) => spawn_expanded_panel(
            &mut commands,
            root,
            *theme,
            &font,
            &model,
            state.page,
            state.scroll_y,
            &view_options,
            &loaded,
        ),
        Err(error) => spawn_panel_error(&mut commands, root, *theme, &font, &error.to_string()),
    }
    queue_panel_focus_restore(
        &mut commands,
        state.restore_focus.take(),
        page_focus_action(state.page),
    );
    state.dirty = false;
}

fn page_focus_action(page: ActivePanelPage) -> PanelAction {
    PanelAction::SelectPage(page)
}

fn queue_panel_focus_restore(
    commands: &mut Commands,
    action: Option<PanelAction>,
    fallback: PanelAction,
) {
    if let Some(action) = action {
        commands.queue(move |world: &mut World| {
            let focused = panel_focus_entity(world, action, fallback);
            if let Some(entity) = focused {
                world
                    .resource_mut::<InputFocus>()
                    .set(entity, FocusCause::Navigated);
            }
        });
    }
}

fn panel_focus_entity(
    world: &mut World,
    requested: PanelAction,
    fallback: PanelAction,
) -> Option<Entity> {
    let mut actions = world.query::<(Entity, &PanelAction)>();
    let exact = actions
        .iter(world)
        .find_map(|(entity, candidate)| (*candidate == requested).then_some(entity));
    exact.or_else(|| {
        actions
            .iter(world)
            .find_map(|(entity, candidate)| (*candidate == fallback).then_some(entity))
    })
}

fn spawn_panel_root(commands: &mut Commands, theme: UiTheme, collapsed: bool) -> Entity {
    commands
        .spawn((
            Name::new("Contextual body panel"),
            LeftPanelRoot,
            HudSurface,
            AccessibleLabel::new("Contextual body information and view options"),
            TabGroup::new(PANEL_TAB_GROUP_ORDER),
            Node {
                position_type: PositionType::Absolute,
                top: px(TOP_BAR_HEIGHT_PX + PANEL_MARGIN_PX),
                bottom: px(TIME_BAR_HEIGHT_PX + PANEL_MARGIN_PX),
                left: px(PANEL_MARGIN_PX),
                width: px(if collapsed {
                    PANEL_COLLAPSED_SIZE_PX
                } else {
                    PANEL_WIDTH_PX
                }),
                max_width: if collapsed { auto() } else { percent(78) },
                height: if collapsed {
                    px(PANEL_COLLAPSED_SIZE_PX)
                } else {
                    auto()
                },
                padding: if collapsed {
                    UiRect::ZERO
                } else {
                    UiRect::all(px(theme.spacing.md_px))
                },
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(theme.colors.panel.color()),
            BorderColor::all(theme.colors.separator.color()),
            GlobalZIndex(PANEL_Z_INDEX),
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
fn spawn_expanded_panel(
    commands: &mut Commands,
    root: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    model: &BodyInfoViewModel,
    page: ActivePanelPage,
    scroll_y: f32,
    settings: &ViewOptionsState,
    loaded: &LoadedCatalog,
) {
    let header = commands
        .spawn((
            Node {
                width: percent(100),
                min_height: px(34),
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.sm_px),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    spawn_text(
        commands,
        header,
        font,
        &model.body.name,
        theme.type_scale.title_px,
        theme.colors.text_primary.color(),
        true,
    );
    spawn_action_button(
        commands,
        header,
        theme,
        font,
        "‹",
        "Collapse body information panel",
        PanelAction::ToggleCollapsed,
        false,
    );

    let scroll_position = ScrollPosition(Vec2::new(0.0, scroll_y));
    let content = commands
        .spawn((
            LeftPanelContent,
            UiScrollSurface,
            AccessibleLabel::new(format!("{} panel content", model.body.name)),
            Node {
                width: percent(100),
                flex_grow: 1.0,
                min_height: px(0),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.spacing.sm_px),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            scroll_position,
            ChildOf(root),
        ))
        .observe(scroll_left_panel_content)
        .id();

    let tabs = commands
        .spawn((
            Node {
                width: percent(100),
                min_height: px(34),
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.xs_px),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    for tab in &model.tabs {
        let tab_page = match tab {
            LeftPanelTab::Info => ActivePanelPage::Info,
            LeftPanelTab::Collection => ActivePanelPage::Collection,
            LeftPanelTab::ViewOptions => ActivePanelPage::ViewOptions,
        };
        spawn_action_button(
            commands,
            tabs,
            theme,
            font,
            tab.label(),
            &format!("Show {} for {}", tab.label(), model.body.name),
            PanelAction::SelectPage(tab_page),
            page == tab_page,
        );
    }

    match page {
        ActivePanelPage::Info => spawn_info_page(commands, content, theme, font, model),
        ActivePanelPage::Collection => {
            if let Some(collection) = &model.collection {
                spawn_collection_page(commands, content, theme, font, collection);
            } else {
                spawn_info_page(commands, content, theme, font, model);
            }
        }
        ActivePanelPage::ViewOptions => {
            spawn_view_options_page(commands, content, theme, font, model, settings, loaded);
        }
    }
}

fn spawn_info_page(
    commands: &mut Commands,
    content: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    model: &BodyInfoViewModel,
) {
    let category = commands
        .spawn((
            Node {
                height: px(28),
                padding: UiRect::horizontal(px(theme.spacing.sm_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::MAX,
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.sm_px),
                ..default()
            },
            AccessibleLabel::new(format!("Category: {}", model.category_label)),
            BackgroundColor(theme.colors.panel_elevated.color()),
            BorderColor::all(theme.colors.separator.color()),
            ChildOf(content),
        ))
        .id();
    let (red, green, blue) = model.category_color_srgb;
    commands.spawn((
        Node {
            width: px(8),
            height: px(8),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(Color::srgb_u8(red, green, blue)),
        ChildOf(category),
    ));
    spawn_text(
        commands,
        category,
        font,
        &model.category_label.to_uppercase(),
        theme.type_scale.caption_px,
        theme.colors.text_primary.color(),
        false,
    );

    spawn_detail_row(
        commands,
        content,
        theme,
        font,
        "RADIUS",
        &model.radius_label,
    );
    if let OrbitalPeriodViewModel::Elliptic { label, .. } = &model.orbital_period {
        spawn_detail_row(commands, content, theme, font, "ORBITAL PERIOD", label);
    }
    if let Some(parent) = &model.parent {
        let row = spawn_detail_row_container(commands, content, theme, font, "PARENT");
        spawn_action_button(
            commands,
            row,
            theme,
            font,
            &parent.name,
            &format!("Travel to parent {}", parent.name),
            PanelAction::TravelTo(parent.body_index),
            false,
        );
    }

    commands
        .spawn_scene(section_header(
            theme,
            WidgetSpec::new(
                "DESCRIPTION",
                format!("Description of {}", model.body.name),
                WidgetVisualState::Default,
            ),
        ))
        .insert(ChildOf(content));
    let description_color = if model.description.is_catalog_lint() {
        theme.colors.accent.color()
    } else {
        theme.colors.text_muted.color()
    };
    let description = spawn_wrapped_text(
        commands,
        content,
        font,
        model.description.text(),
        theme.type_scale.body_px,
        description_color,
    );
    commands
        .entity(description)
        .insert(AccessibleLabel::new(model.description.text().to_string()));

    if let Some(collection) = &model.collection {
        spawn_action_button(
            commands,
            content,
            theme,
            font,
            &format!("{}  →", collection.label),
            &format!("Open {}", collection.label),
            PanelAction::SelectPage(ActivePanelPage::Collection),
            false,
        );
    }
}

fn spawn_collection_page(
    commands: &mut Commands,
    content: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    collection: &MoonCollectionViewModel,
) {
    commands
        .spawn_scene(section_header(
            theme,
            WidgetSpec::new(
                collection.label.to_uppercase(),
                collection.label.clone(),
                WidgetVisualState::Active,
            ),
        ))
        .insert(ChildOf(content));
    for child in &collection.children {
        spawn_action_button(
            commands,
            content,
            theme,
            font,
            &format!("{}  →", child.name),
            &format!("Select and travel to {}", child.name),
            PanelAction::TravelTo(child.body_index),
            false,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_view_options_page(
    commands: &mut Commands,
    content: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    model: &BodyInfoViewModel,
    settings: &ViewOptionsState,
    loaded: &LoadedCatalog,
) {
    commands
        .spawn_scene(section_header(
            theme,
            WidgetSpec::new(
                "BODY SIZE",
                "Visual body size exaggeration",
                WidgetVisualState::Default,
            ),
        ))
        .insert(ChildOf(content));
    let sizes = spawn_button_group(commands, content, theme);
    for scale in BodySizeScale::ALL {
        spawn_action_button(
            commands,
            sizes,
            theme,
            font,
            scale.label(),
            &format!("Set visual body sizes to {}", scale.label()),
            PanelAction::SetBodySize(scale),
            settings.body_size() == scale,
        );
    }
    spawn_wrapped_text(
        commands,
        content,
        font,
        "Visual scale only. Propagation and picking remain true-radius.",
        theme.type_scale.caption_px,
        theme.colors.text_muted.color(),
    );

    if let Some((system_index, system)) =
        moon_system_index(loaded, model.body.body_index).and_then(|system_index| {
            loaded
                .catalog
                .bodies
                .get(system_index)
                .map(|system| (system_index, system))
        })
    {
        commands
            .spawn_scene(section_header(
                theme,
                WidgetSpec::new(
                    "MOON VISIBILITY",
                    format!("Moon visibility for {}", system.name),
                    WidgetVisualState::Default,
                ),
            ))
            .insert(ChildOf(content));
        let modes = spawn_button_group(commands, content, theme);
        spawn_action_button(
            commands,
            modes,
            theme,
            font,
            "MAJOR",
            &format!("Show the curated major moons of {}", system.name),
            PanelAction::SetMoonVisibility {
                system_index,
                mode: MoonVisibilityMode::Major,
            },
            settings.moon_visibility(&system.id) == MoonVisibilityMode::Major,
        );
        spawn_action_button(
            commands,
            modes,
            theme,
            font,
            "ALL",
            &format!("Show all modeled moons of {}", system.name),
            PanelAction::SetMoonVisibility {
                system_index,
                mode: MoonVisibilityMode::All,
            },
            settings.moon_visibility(&system.id) == MoonVisibilityMode::All,
        );
    }

    commands
        .spawn_scene(section_header(
            theme,
            WidgetSpec::new(
                "LOCAL ORBIT",
                format!("Orbit line for {}", model.body.name),
                WidgetVisualState::Default,
            ),
        ))
        .insert(ChildOf(content));
    if model.category == Category::Star {
        spawn_disabled_button(
            commands,
            content,
            theme,
            font,
            "NO LOCAL ORBIT",
            "The central star has no parent orbit",
        );
    } else {
        let visible = settings.local_orbit_visible(&model.body.id);
        spawn_action_button(
            commands,
            content,
            theme,
            font,
            if visible {
                "✓ ORBIT VISIBLE"
            } else {
                "ORBIT HIDDEN"
            },
            &format!("Toggle the local orbit line for {}", model.body.name),
            PanelAction::ToggleLocalOrbit(model.body.body_index),
            visible,
        );
    }
}

fn moon_system_index(loaded: &LoadedCatalog, body_index: usize) -> Option<usize> {
    let body = loaded.catalog.bodies.get(body_index)?;
    if body.category == Category::Moon {
        body.parent
            .as_deref()
            .and_then(|parent_id| loaded.index_of(parent_id))
    } else {
        loaded
            .catalog
            .bodies
            .iter()
            .any(|candidate| {
                candidate.category == Category::Moon
                    && candidate.parent.as_deref() == Some(body.id.as_str())
            })
            .then_some(body_index)
    }
}

fn spawn_panel_error(
    commands: &mut Commands,
    root: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    error: &str,
) {
    spawn_wrapped_text(
        commands,
        root,
        font,
        &format!("INFO MODEL ERROR\n{error}"),
        theme.type_scale.body_px,
        theme.colors.accent.color(),
    );
}

fn spawn_detail_row(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    label: &str,
    value: &str,
) {
    let row = spawn_detail_row_container(commands, parent, theme, font, label);
    spawn_text(
        commands,
        row,
        font,
        value,
        theme.type_scale.body_px,
        theme.colors.text_primary.color(),
        false,
    );
}

fn spawn_detail_row_container(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    label: &str,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: percent(100),
                min_height: px(30),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(theme.spacing.sm_px),
                ..default()
            },
            ChildOf(parent),
        ))
        .id();
    spawn_text(
        commands,
        row,
        font,
        label,
        theme.type_scale.caption_px,
        theme.colors.text_muted.color(),
        false,
    );
    row
}

fn spawn_button_group(commands: &mut Commands, parent: Entity, theme: UiTheme) -> Entity {
    commands
        .spawn((
            Node {
                width: percent(100),
                min_height: px(34),
                align_items: AlignItems::Center,
                column_gap: px(theme.spacing.xs_px),
                ..default()
            },
            ChildOf(parent),
        ))
        .id()
}

#[allow(clippy::too_many_arguments)]
fn spawn_action_button(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    text: &str,
    accessible_label: &str,
    action: PanelAction,
    active: bool,
) -> Entity {
    let compact = matches!(action, PanelAction::ToggleCollapsed);
    let entity = commands
        .spawn((
            bevy::ui_widgets::Button,
            action,
            AccessibleLabel::new(accessible_label),
            TabIndex(panel_tab_index(action)),
            Node {
                min_width: px(34),
                max_width: if compact { px(44) } else { auto() },
                height: px(32),
                max_height: px(34),
                min_height: px(30),
                padding: UiRect::horizontal(px(theme.spacing.sm_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(if active {
                Color::srgba_u8(
                    theme.colors.accent.0[0],
                    theme.colors.accent.0[1],
                    theme.colors.accent.0[2],
                    38,
                )
            } else {
                theme.colors.panel_elevated.color()
            }),
            BorderColor::all(if active {
                theme.colors.accent.color()
            } else {
                theme.colors.separator.color()
            }),
            ChildOf(parent),
        ))
        .observe(activate_panel_action)
        .id();
    spawn_text(
        commands,
        entity,
        font,
        text,
        theme.type_scale.caption_px,
        if active {
            theme.colors.text_primary.color()
        } else {
            theme.colors.text_muted.color()
        },
        false,
    );
    entity
}

fn panel_tab_index(action: PanelAction) -> i32 {
    match action {
        PanelAction::ToggleCollapsed => 0,
        PanelAction::SelectPage(ActivePanelPage::Info) => 10,
        PanelAction::SelectPage(ActivePanelPage::Collection) => 11,
        PanelAction::SelectPage(ActivePanelPage::ViewOptions) => 12,
        PanelAction::TravelTo(body_index) => 100 + body_index as i32,
        PanelAction::SetBodySize(BodySizeScale::X1) => 200,
        PanelAction::SetBodySize(BodySizeScale::X10) => 201,
        PanelAction::SetBodySize(BodySizeScale::X50) => 202,
        PanelAction::SetMoonVisibility {
            mode: MoonVisibilityMode::Major,
            ..
        } => 210,
        PanelAction::SetMoonVisibility {
            mode: MoonVisibilityMode::All,
            ..
        } => 211,
        PanelAction::ToggleLocalOrbit(_) => 220,
    }
}

fn spawn_disabled_button(
    commands: &mut Commands,
    parent: Entity,
    theme: UiTheme,
    font: &Handle<Font>,
    text: &str,
    accessible_label: &str,
) -> Entity {
    let entity = commands
        .spawn((
            AccessibleLabel::new(accessible_label),
            Node {
                min_height: px(30),
                padding: UiRect::horizontal(px(theme.spacing.sm_px)),
                border: UiRect::all(px(theme.spacing.hairline_px)),
                border_radius: BorderRadius::all(px(theme.spacing.radius_px)),
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(theme.colors.background.color()),
            BorderColor::all(theme.colors.separator.color()),
            ChildOf(parent),
        ))
        .id();
    spawn_text(
        commands,
        entity,
        font,
        text,
        theme.type_scale.caption_px,
        theme.colors.text_disabled.color(),
        false,
    );
    entity
}

fn spawn_text(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    text: &str,
    size: f32,
    color: Color,
    grow: bool,
) -> Entity {
    commands
        .spawn((
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: size.into(),
                ..default()
            },
            TextColor(color),
            LetterSpacing::Px(0.0),
            Node {
                flex_grow: if grow { 1.0 } else { 0.0 },
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

fn spawn_wrapped_text(
    commands: &mut Commands,
    parent: Entity,
    font: &Handle<Font>,
    text: &str,
    size: f32,
    color: Color,
) -> Entity {
    commands
        .spawn((
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: size.into(),
                ..default()
            },
            TextColor(color),
            TextLayout {
                linebreak: LineBreak::WordBoundary,
                ..default()
            },
            Node {
                width: percent(100),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
}

fn scroll_left_panel_content(
    mut scroll: On<Pointer<Scroll>>,
    mut contents: Query<(&mut ScrollPosition, &ComputedNode), With<LeftPanelContent>>,
    mut state: ResMut<LeftPanelUiState>,
) {
    let Ok((mut position, node)) = contents.get_mut(scroll.entity) else {
        return;
    };
    position.y = next_panel_scroll_y(
        position.y,
        scroll.y,
        scroll.unit,
        node.content_size().y,
        node.size().y,
        node.inverse_scale_factor,
    );
    state.scroll_y = position.y;
    scroll.propagate(false);
}

fn next_panel_scroll_y(
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

fn activate_panel_action(
    activate: On<Activate>,
    actions: Query<&PanelAction>,
    loaded: Res<LoadedCatalog>,
    settings: Res<ViewOptionsState>,
    focus: Res<InputFocus>,
    mut state: ResMut<LeftPanelUiState>,
    mut sim_commands: ResMut<SimCommandQueue>,
) {
    let Ok(action) = actions.get(activate.entity) else {
        return;
    };
    if focus.get() == Some(activate.entity) {
        state.restore_focus = Some(*action);
    }
    match *action {
        PanelAction::ToggleCollapsed => {
            let collapsed = !settings.panel_collapsed();
            sim_commands.push(SimCommand::SetLeftPanelCollapsed(collapsed));
        }
        PanelAction::SelectPage(page) => {
            let tab = match page {
                ActivePanelPage::Info => LeftPanelTab::Info,
                ActivePanelPage::Collection => LeftPanelTab::Collection,
                ActivePanelPage::ViewOptions => LeftPanelTab::ViewOptions,
            };
            sim_commands.push(SimCommand::SetLeftPanelTab(tab));
        }
        PanelAction::TravelTo(body_index) => {
            if let Some(body) = loaded.catalog.bodies.get(body_index) {
                sim_commands.push(SimCommand::TravelToBody(body.id.clone()));
            }
        }
        PanelAction::SetBodySize(scale) => {
            sim_commands.push(SimCommand::SetBodySize(scale));
        }
        PanelAction::SetMoonVisibility { system_index, mode } => {
            if let Some(system) = loaded.catalog.bodies.get(system_index) {
                sim_commands.push(SimCommand::SetMoonVisibility {
                    system_id: system.id.clone(),
                    mode,
                });
            }
        }
        PanelAction::ToggleLocalOrbit(body_index) => {
            if let Some(body) = loaded.catalog.bodies.get(body_index) {
                let visible = !settings.local_orbit_visible(&body.id);
                sim_commands.push(SimCommand::SetLocalOrbitVisibility {
                    body_id: body.id.clone(),
                    visible,
                });
            }
        }
    }
}

fn apply_view_options(
    settings: Res<ViewOptionsState>,
    layers: Option<Res<LayerState>>,
    camera: Option<Res<CameraController>>,
    loaded: Option<Res<LoadedCatalog>>,
    mut previous_focus: Local<Option<Option<usize>>>,
    mut bodies: Query<(&BodyVisual, Option<&mut Visibility>)>,
) {
    let focus = camera.as_ref().map(|camera| camera.focus_body_index());
    if !body_presentation_inputs_changed(
        settings.is_changed(),
        layers.as_ref().is_some_and(|layers| layers.is_changed()),
        focus,
        &mut previous_focus,
    ) {
        return;
    }
    let Some(loaded) = loaded else {
        return;
    };
    for (visual, visibility) in &mut bodies {
        if let Some(body) = loaded.catalog.bodies.get(visual.index) {
            if let Some(mut visibility) = visibility {
                let category_visible = layers
                    .as_ref()
                    .is_none_or(|layers| layers.body_category_visible(body.category));
                let moon_visible = body.category != Category::Moon
                    || focus.is_some_and(|focus| {
                        body_passes_contextual_moon_visibility(
                            visual.index,
                            focus,
                            &loaded,
                            Some(&settings),
                        )
                    });
                let desired_visibility = if category_visible && moon_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *visibility != desired_visibility {
                    *visibility = desired_visibility;
                }
            }
        }
    }
}

fn body_presentation_inputs_changed(
    settings_changed: bool,
    layers_changed: bool,
    focus: Option<usize>,
    previous_focus: &mut Option<Option<usize>>,
) -> bool {
    let focus_changed = previous_focus.is_none_or(|previous| previous != focus);
    *previous_focus = Some(focus);
    settings_changed || layers_changed || focus_changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::{inflated_pick_radius, ray_sphere_hit_distance};
    use crate::ui_kit::test_layout;
    use crate::{load_catalog_text, propagate_catalog, HeadlessSimulation};
    use bevy::{
        app::TaskPoolPlugin,
        asset::{AssetApp, AssetPlugin},
        camera::PerspectiveProjection,
        scene::ScenePlugin,
        text::Font,
    };
    use std::collections::HashSet;

    #[derive(Resource, Debug, Default)]
    struct BodyPresentationWrites {
        transforms: usize,
        visibility: usize,
    }

    #[derive(Resource, Debug, Default)]
    struct NavigationSyncChanges {
        panel: usize,
        navigation: usize,
    }

    fn count_body_presentation_writes(
        transforms: Query<Entity, (With<BodyVisual>, Changed<Transform>)>,
        visibility: Query<Entity, (With<BodyVisual>, Changed<Visibility>)>,
        mut writes: ResMut<BodyPresentationWrites>,
    ) {
        writes.transforms = transforms.iter().count();
        writes.visibility = visibility.iter().count();
    }

    fn count_navigation_sync_changes(
        panel: Res<LeftPanelUiState>,
        navigation: Res<NavigationStack>,
        mut changes: ResMut<NavigationSyncChanges>,
    ) {
        changes.panel = usize::from(panel.is_changed());
        changes.navigation = usize::from(navigation.is_changed());
    }

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    fn catalog() -> Catalog {
        load_catalog_text(REAL_CATALOG).unwrap()
    }

    fn rendered_panel_app(selected_body_index: usize) -> App {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .insert_resource(LoadedCatalog::new(catalog()))
        .insert_resource(ViewOptionsState::default())
        .insert_resource(AppSettings::default())
        .init_resource::<InputFocus>()
        .insert_resource(LeftPanelUiState {
            selected_body_index: Some(selected_body_index),
            page: ActivePanelPage::Info,
            dirty: true,
            scroll_y: 0.0,
            reset_scroll_on_rebuild: false,
            restore_focus: None,
            ..default()
        })
        .add_systems(Update, rebuild_left_panel);
        app.update();
        app
    }

    #[test]
    fn duplicate_view_and_navigation_commands_leave_panel_models_identical() {
        let loaded = LoadedCatalog::new(catalog());
        let jupiter = loaded.index_of("jupiter").unwrap();
        let mut view_options = ViewOptionsState::default();
        let mut state = LeftPanelUiState::default();
        let mut navigation = NavigationStack::root();
        apply_body_selection(&loaded, jupiter, &mut state, &mut navigation);
        state.dirty = false;
        state.scroll_y = 73.0;
        state.reset_scroll_on_rebuild = false;

        let view_before = view_options.clone();
        let state_before = state.clone();
        let navigation_before = navigation.clone();
        for command in [
            SimCommand::SetBodySize(BodySizeScale::X1),
            SimCommand::SetMoonVisibility {
                system_id: "jupiter".into(),
                mode: MoonVisibilityMode::All,
            },
            SimCommand::SetLocalOrbitVisibility {
                body_id: "jupiter".into(),
                visible: true,
            },
            SimCommand::SetLeftPanelCollapsed(false),
            SimCommand::SetLeftPanelTab(LeftPanelTab::Info),
            SimCommand::SelectBody("jupiter".into()),
            SimCommand::NavigateBreadcrumb {
                depth: 1,
                target_id: "jupiter".into(),
            },
            SimCommand::RestorePresentationDefaults,
        ] {
            consume_left_panel_command(
                &command,
                Some(&loaded),
                &mut view_options,
                &mut state,
                &mut navigation,
            );
            assert_eq!(view_options, view_before, "{command:?}");
            assert_eq!(state, state_before, "{command:?}");
            assert_eq!(navigation, navigation_before, "{command:?}");
        }
    }

    #[test]
    fn stable_selection_sync_does_not_mark_panel_or_navigation_changed() {
        let loaded = LoadedCatalog::new(catalog());
        let jupiter = loaded.index_of("jupiter").unwrap();
        let mut state = LeftPanelUiState::default();
        let mut navigation = NavigationStack::root();
        apply_body_selection(&loaded, jupiter, &mut state, &mut navigation);
        state.dirty = false;

        let mut app = App::new();
        app.insert_resource(loaded)
            .insert_resource(CameraController::new(jupiter, [0.0; 3], 1.0))
            .insert_resource(state)
            .insert_resource(navigation)
            .init_resource::<NavigationSyncChanges>()
            .add_systems(
                Update,
                (
                    sync_left_panel_selection,
                    count_navigation_sync_changes.after(sync_left_panel_selection),
                ),
            );
        app.update();

        *app.world_mut().resource_mut::<NavigationSyncChanges>() = NavigationSyncChanges::default();
        app.update();

        let changes = app.world().resource::<NavigationSyncChanges>();
        assert_eq!(changes.panel, 0);
        assert_eq!(changes.navigation, 0);
    }

    #[test]
    fn stable_body_presentation_values_do_not_rewrite_components() {
        let loaded = LoadedCatalog::new(catalog());
        let earth = loaded.index_of("earth").unwrap();
        let body = &loaded.catalog.bodies[earth];
        let desired_scale = Vec3::splat(rendered_body_radius_units(
            body.radius_km,
            BodySizeScale::X1,
        ));
        let mut app = App::new();
        app.insert_resource(loaded)
            .insert_resource(ViewOptionsState::default())
            .insert_resource(LayerState::default())
            .init_resource::<BodyPresentationWrites>()
            .add_systems(
                Update,
                (
                    apply_view_options,
                    count_body_presentation_writes.after(apply_view_options),
                ),
            );
        app.world_mut().spawn((
            BodyVisual { index: earth },
            Transform::from_scale(desired_scale),
            Visibility::Visible,
        ));
        app.update();

        app.world_mut()
            .resource_mut::<BodyPresentationWrites>()
            .transforms = 0;
        app.world_mut()
            .resource_mut::<BodyPresentationWrites>()
            .visibility = 0;
        let _ = app
            .world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_body_size(BodySizeScale::X1);
        app.update();

        let writes = app.world().resource::<BodyPresentationWrites>();
        assert_eq!(writes.transforms, 0);
        assert_eq!(writes.visibility, 0);
    }

    #[test]
    fn camera_pose_changes_do_not_rescan_body_presentation_without_a_focus_change() {
        let mut previous_focus = None;
        assert!(body_presentation_inputs_changed(
            false,
            false,
            Some(3),
            &mut previous_focus
        ));
        assert!(!body_presentation_inputs_changed(
            false,
            false,
            Some(3),
            &mut previous_focus
        ));
        assert!(body_presentation_inputs_changed(
            false,
            false,
            Some(4),
            &mut previous_focus
        ));
        assert!(body_presentation_inputs_changed(
            true,
            false,
            Some(4),
            &mut previous_focus
        ));
        assert!(body_presentation_inputs_changed(
            false,
            true,
            Some(4),
            &mut previous_focus
        ));
    }

    fn descendant_of(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
        for _ in 0..16 {
            if entity == ancestor {
                return true;
            }
            let Some(parent) = world.entity(entity).get::<ChildOf>() else {
                return false;
            };
            entity = parent.parent();
        }
        false
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

    fn layout_rect_contains(outer: Rect, inner: Rect) -> bool {
        inner.min.x >= outer.min.x - 1.0
            && inner.max.x <= outer.max.x + 1.0
            && inner.min.y >= outer.min.y - 1.0
            && inner.max.y <= outer.max.y + 1.0
    }

    #[test]
    fn all_sixty_six_info_models_are_render_ready() {
        let catalog = catalog();
        let lints = catalog.lint();
        assert_eq!(catalog.bodies.len(), 66);
        assert!(lints.is_empty(), "committed catalog lints: {lints:?}");

        for (index, body) in catalog.bodies.iter().enumerate() {
            let model = body_info_view_model(&catalog, index)
                .unwrap_or_else(|error| panic!("{} failed: {error}", body.id));
            assert!(!model.body.name.trim().is_empty());
            assert!(!model.category_label.trim().is_empty());
            assert!(model.radius_km > 0.0);
            assert!(!model.radius_label.trim().is_empty());
            match (&body.orbit, &model.orbital_period) {
                (None, OrbitalPeriodViewModel::NotApplicable) => {}
                (Some(orbit), OrbitalPeriodViewModel::Hyperbolic)
                    if orbit.elements.is_hyperbolic() => {}
                (Some(orbit), OrbitalPeriodViewModel::Elliptic { seconds, label })
                    if !orbit.elements.is_hyperbolic() =>
                {
                    assert!(seconds.is_finite() && *seconds > 0.0);
                    assert!(!label.trim().is_empty());
                }
                combination => panic!("{} has mismatched period {combination:?}", body.id),
            }
            assert!(!model.description.text().trim().is_empty());
            if body.description.trim().is_empty() {
                assert!(model.description.is_catalog_lint());
                assert!(lints.iter().any(|lint| lint == model.description.text()));
            } else {
                assert_eq!(model.description.text(), body.description.trim());
            }
        }
    }

    #[test]
    fn units_change_rebuilds_every_visible_radius_within_one_update() {
        let loaded = LoadedCatalog::new(catalog());
        let earth = loaded.index_of("earth").unwrap();
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
        ))
        .init_asset::<Font>()
        .insert_resource(UiTheme::default())
        .insert_resource(loaded)
        .insert_resource(ViewOptionsState::default())
        .insert_resource(AppSettings::default())
        .insert_resource(LeftPanelUiState {
            selected_body_index: Some(earth),
            page: ActivePanelPage::Info,
            dirty: true,
            scroll_y: 0.0,
            reset_scroll_on_rebuild: false,
            restore_focus: None,
            ..default()
        })
        .add_systems(Update, rebuild_left_panel);
        app.update();
        assert!(visible_text_contains(&mut app, "6371 km"));

        app.world_mut().resource_mut::<AppSettings>().units = DistanceUnit::Miles;
        app.update();
        assert!(visible_text_contains(&mut app, "3959 mi"));
        assert!(!visible_text_contains(&mut app, "6371 km"));
    }

    #[test]
    fn unrelated_settings_changes_retain_the_left_panel_surface() {
        let loaded = LoadedCatalog::new(catalog());
        let earth = loaded.index_of("earth").unwrap();
        let mut app = rendered_panel_app(earth);
        let root = {
            let world = app.world_mut();
            world
                .query_filtered::<Entity, With<LeftPanelRoot>>()
                .single(world)
                .unwrap()
        };

        {
            let mut settings = app.world_mut().resource_mut::<AppSettings>();
            settings.layers.icons = false;
            settings.invert_horizontal = true;
            settings.start_mode = crate::StartModeSetting::Live;
        }
        let _ = app
            .world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_body_size(BodySizeScale::X1);
        app.update();

        let retained = app
            .world_mut()
            .query_filtered::<Entity, With<LeftPanelRoot>>()
            .single(app.world())
            .unwrap();
        assert_eq!(retained, root);
    }

    #[test]
    fn expanded_panel_is_an_ordered_tab_group_with_tabs_inside_scroll_content() {
        let loaded = LoadedCatalog::new(catalog());
        let earth = loaded.index_of("earth").unwrap();
        let mut app = rendered_panel_app(earth);
        let world = app.world_mut();
        let root = world
            .query_filtered::<Entity, With<LeftPanelRoot>>()
            .single(world)
            .unwrap();
        let group = world.entity(root).get::<TabGroup>().unwrap();
        let node = world.entity(root).get::<Node>().unwrap();
        assert_eq!(group.order, PANEL_TAB_GROUP_ORDER);
        assert!(!group.modal);
        assert_eq!(node.max_width, percent(78));

        let content = world
            .query_filtered::<Entity, With<LeftPanelContent>>()
            .single(world)
            .unwrap();
        assert!(descendant_of(world, content, root));
        let page_actions: Vec<_> = world
            .query::<(Entity, &PanelAction)>()
            .iter(world)
            .filter_map(|(entity, action)| {
                matches!(action, PanelAction::SelectPage(_)).then_some(entity)
            })
            .collect();
        assert!(!page_actions.is_empty());
        assert!(page_actions
            .into_iter()
            .all(|entity| descendant_of(world, entity, content)));
    }

    #[test]
    fn left_panel_reaches_last_action_for_required_viewports() {
        for (width, height, scale) in test_layout::required_viewports() {
            let loaded = LoadedCatalog::new(catalog());
            let jupiter = loaded.index_of("jupiter").unwrap();
            let mut app = test_layout::app(width, height, scale);
            app.insert_resource(UiTheme::default())
                .insert_resource(loaded)
                .insert_resource(ViewOptionsState::default())
                .insert_resource(AppSettings::default())
                .init_resource::<InputFocus>()
                .insert_resource(LeftPanelUiState {
                    selected_body_index: Some(jupiter),
                    page: ActivePanelPage::Collection,
                    dirty: true,
                    scroll_y: 0.0,
                    reset_scroll_on_rebuild: false,
                    restore_focus: None,
                    ..default()
                })
                .add_systems(Update, rebuild_left_panel);
            test_layout::settle(&mut app);

            let root = app
                .world_mut()
                .query_filtered::<Entity, With<LeftPanelRoot>>()
                .single(app.world())
                .unwrap();
            let content = app
                .world_mut()
                .query_filtered::<Entity, With<LeftPanelContent>>()
                .single(app.world())
                .unwrap();
            let root_rect = layout_node_rect(app.world(), root);
            let viewport = Rect::from_corners(Vec2::ZERO, Vec2::new(width as f32, height as f32));
            assert!(
                layout_rect_contains(viewport, root_rect),
                "{width}×{height} scale {scale}: left panel {root_rect:?} escaped viewport"
            );
            assert!(
                root_rect.width() <= width as f32 * 0.78 + 1.0,
                "{width}×{height} scale {scale}: left panel ignored responsive width cap"
            );
            assert!(
                app.world().get::<ComputedNode>(content).unwrap().size().y >= 30.0 * scale,
                "{width}×{height} scale {scale}: left panel cannot expose one action"
            );

            app.world_mut()
                .entity_mut(content)
                .get_mut::<ScrollPosition>()
                .unwrap()
                .y = f32::MAX;
            test_layout::settle(&mut app);
            let last = app
                .world_mut()
                .query::<(Entity, &PanelAction, &TabIndex)>()
                .iter(app.world())
                .filter(|(_, action, _)| matches!(action, PanelAction::TravelTo(_)))
                .max_by_key(|(_, _, index)| index.0)
                .map(|(entity, _, _)| entity)
                .unwrap();
            let last_rect = layout_node_rect(app.world(), last);
            let content_rect = layout_node_rect(app.world(), content);
            assert!(
                layout_rect_contains(content_rect, last_rect),
                "{width}×{height} scale {scale}: final left-panel action {last_rect:?} is not reachable inside {content_rect:?}"
            );
        }
    }

    #[test]
    fn longest_description_wraps_scrolls_and_remains_accessible_at_required_viewports() {
        for (width, height, scale) in test_layout::required_viewports() {
            let loaded = LoadedCatalog::new(catalog());
            let longest = loaded
                .catalog
                .bodies
                .iter()
                .enumerate()
                .max_by_key(|(_, body)| body.description.len())
                .map(|(index, _)| index)
                .unwrap();
            let expected = loaded.catalog.bodies[longest].description.clone();
            let mut app = test_layout::app(width, height, scale);
            app.insert_resource(UiTheme::default())
                .insert_resource(loaded)
                .insert_resource(ViewOptionsState::default())
                .insert_resource(AppSettings::default())
                .init_resource::<InputFocus>()
                .insert_resource(LeftPanelUiState {
                    selected_body_index: Some(longest),
                    page: ActivePanelPage::Info,
                    dirty: true,
                    scroll_y: 0.0,
                    reset_scroll_on_rebuild: false,
                    restore_focus: None,
                    ..default()
                })
                .add_systems(Update, rebuild_left_panel);
            test_layout::settle(&mut app);

            let content = app
                .world_mut()
                .query_filtered::<Entity, With<LeftPanelContent>>()
                .single(app.world())
                .unwrap();
            let description = app
                .world_mut()
                .query::<(Entity, &Text)>()
                .iter(app.world())
                .find_map(|(entity, text)| (text.0 == expected).then_some(entity))
                .unwrap();
            assert_eq!(
                app.world()
                    .get::<TextLayout>(description)
                    .unwrap()
                    .linebreak,
                LineBreak::WordBoundary
            );
            assert_eq!(
                app.world().get::<AccessibleLabel>(description).unwrap().0,
                expected
            );
            assert!(
                layout_node_rect(app.world(), description).height()
                    > UiTheme::default().type_scale.body_px * scale * 1.5,
                "{width}×{height} scale {scale}: longest description did not wrap"
            );

            let description_rect = layout_node_rect(app.world(), description);
            let content_rect = layout_node_rect(app.world(), content);
            app.world_mut()
                .entity_mut(content)
                .get_mut::<ScrollPosition>()
                .unwrap()
                .y += (description_rect.min.y - content_rect.min.y) / scale;
            test_layout::settle(&mut app);
            let description_top = layout_node_rect(app.world(), description);
            let content_rect = layout_node_rect(app.world(), content);
            assert!(
                description_top.min.y >= content_rect.min.y - 1.0
                    && description_top.min.y <= content_rect.max.y + 1.0
                    && description_top.min.x >= content_rect.min.x - 1.0
                    && description_top.max.x <= content_rect.max.x + 1.0,
                "{width}×{height} scale {scale}: description top {description_top:?} is not scroll-reachable inside {content_rect:?}"
            );

            app.world_mut()
                .entity_mut(content)
                .get_mut::<ScrollPosition>()
                .unwrap()
                .y = f32::MAX;
            test_layout::settle(&mut app);
            let description_rect = layout_node_rect(app.world(), description);
            let content_rect = layout_node_rect(app.world(), content);
            assert!(
                description_rect.max.y >= content_rect.min.y - 1.0
                    && description_rect.max.y <= content_rect.max.y + 1.0
                    && description_rect.min.x >= content_rect.min.x - 1.0
                    && description_rect.max.x <= content_rect.max.x + 1.0,
                "{width}×{height} scale {scale}: description bottom {description_rect:?} is not scroll-reachable inside {content_rect:?}"
            );
        }
    }

    #[test]
    fn panel_actions_use_unique_stable_semantic_tab_indices() {
        let actions = [
            PanelAction::ToggleCollapsed,
            PanelAction::SelectPage(ActivePanelPage::Info),
            PanelAction::SelectPage(ActivePanelPage::Collection),
            PanelAction::SelectPage(ActivePanelPage::ViewOptions),
            PanelAction::TravelTo(0),
            PanelAction::TravelTo(65),
            PanelAction::SetBodySize(BodySizeScale::X1),
            PanelAction::SetBodySize(BodySizeScale::X10),
            PanelAction::SetBodySize(BodySizeScale::X50),
            PanelAction::SetMoonVisibility {
                system_index: 5,
                mode: MoonVisibilityMode::Major,
            },
            PanelAction::SetMoonVisibility {
                system_index: 5,
                mode: MoonVisibilityMode::All,
            },
            PanelAction::ToggleLocalOrbit(5),
        ];
        let indices: HashSet<_> = actions.into_iter().map(panel_tab_index).collect();
        assert_eq!(indices.len(), actions.len());
        assert_eq!(panel_tab_index(PanelAction::TravelTo(42)), 142);
    }

    #[test]
    fn focused_panel_actions_survive_rebuilds_and_missing_travel_uses_info_fallback() {
        let loaded = LoadedCatalog::new(catalog());
        let earth = loaded.index_of("earth").unwrap();
        let sun = loaded.index_of("sun").unwrap();
        let mut app = rendered_panel_app(earth);
        {
            let mut state = app.world_mut().resource_mut::<LeftPanelUiState>();
            state.page = ActivePanelPage::ViewOptions;
            state.dirty = true;
        }
        app.update();

        let size_button = {
            let world = app.world_mut();
            world
                .query::<(Entity, &PanelAction)>()
                .iter(world)
                .find_map(|(entity, action)| {
                    (*action == PanelAction::SetBodySize(BodySizeScale::X10)).then_some(entity)
                })
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(size_button, FocusCause::Navigated);
        app.world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_body_size(BodySizeScale::X10);
        {
            let scroll = {
                let world = app.world_mut();
                world
                    .query_filtered::<Entity, With<LeftPanelContent>>()
                    .single(world)
                    .unwrap()
            };
            app.world_mut()
                .entity_mut(scroll)
                .get_mut::<ScrollPosition>()
                .unwrap()
                .y = 37.0;
        }
        app.update();

        let rebuilt_size_button = {
            let world = app.world_mut();
            world
                .query::<(Entity, &PanelAction)>()
                .iter(world)
                .find_map(|(entity, action)| {
                    (*action == PanelAction::SetBodySize(BodySizeScale::X10)).then_some(entity)
                })
                .unwrap()
        };
        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(rebuilt_size_button)
        );
        let scroll_y = {
            let world = app.world_mut();
            world
                .query_filtered::<&ScrollPosition, With<LeftPanelContent>>()
                .single(world)
                .unwrap()
                .y
        };
        assert_eq!(scroll_y, 37.0);

        {
            let mut state = app.world_mut().resource_mut::<LeftPanelUiState>();
            state.page = ActivePanelPage::Info;
            state.dirty = true;
        }
        app.update();
        let parent_button = {
            let world = app.world_mut();
            world
                .query::<(Entity, &PanelAction)>()
                .iter(world)
                .find_map(|(entity, action)| {
                    (*action == PanelAction::TravelTo(sun)).then_some(entity)
                })
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(parent_button, FocusCause::Navigated);
        {
            let mut state = app.world_mut().resource_mut::<LeftPanelUiState>();
            state.selected_body_index = Some(sun);
            state.page = ActivePanelPage::Info;
            state.dirty = true;
        }
        app.update();

        let focused = app.world().resource::<InputFocus>().get().unwrap();
        assert_eq!(
            app.world().entity(focused).get::<PanelAction>(),
            Some(&PanelAction::SelectPage(ActivePanelPage::Info))
        );
    }

    #[test]
    fn semantic_page_and_body_transitions_keep_their_intentional_scroll_reset() {
        let loaded = LoadedCatalog::new(catalog());
        let earth = loaded.index_of("earth").unwrap();
        let jupiter = loaded.index_of("jupiter").unwrap();
        let mut state = LeftPanelUiState {
            selected_body_index: Some(jupiter),
            page: ActivePanelPage::Info,
            dirty: false,
            scroll_y: 81.0,
            reset_scroll_on_rebuild: false,
            restore_focus: None,
            ..default()
        };
        let mut view_options = ViewOptionsState::default();
        let mut navigation = NavigationStack::root();

        consume_left_panel_command(
            &SimCommand::SetLeftPanelTab(LeftPanelTab::Collection),
            Some(&loaded),
            &mut view_options,
            &mut state,
            &mut navigation,
        );
        assert_eq!(state.scroll_y, 0.0);
        assert!(state.reset_scroll_on_rebuild);

        state.scroll_y = 52.0;
        state.reset_scroll_on_rebuild = false;
        consume_left_panel_command(
            &SimCommand::NavigateBreadcrumb {
                depth: 1,
                target_id: "jupiter".into(),
            },
            Some(&loaded),
            &mut view_options,
            &mut state,
            &mut navigation,
        );
        assert_eq!(state.scroll_y, 0.0);
        assert!(state.reset_scroll_on_rebuild);

        state.scroll_y = 29.0;
        state.reset_scroll_on_rebuild = false;
        sync_left_panel_selection_state(
            &CameraController::new(earth, [0.0; 3], 1.0),
            &loaded,
            &mut state,
            &mut navigation,
        );
        assert_eq!(state.scroll_y, 0.0);
        assert!(state.reset_scroll_on_rebuild);

        let mut app = rendered_panel_app(jupiter);
        let content = {
            let world = app.world_mut();
            world
                .query_filtered::<Entity, With<LeftPanelContent>>()
                .single(world)
                .unwrap()
        };
        app.world_mut()
            .entity_mut(content)
            .get_mut::<ScrollPosition>()
            .unwrap()
            .y = 97.0;
        {
            let mut state = app.world_mut().resource_mut::<LeftPanelUiState>();
            state.page = ActivePanelPage::Collection;
            state.dirty = true;
            state.scroll_y = 0.0;
            state.reset_scroll_on_rebuild = true;
        }
        app.update();

        let rebuilt_scroll_y = {
            let world = app.world_mut();
            world
                .query_filtered::<&ScrollPosition, With<LeftPanelContent>>()
                .single(world)
                .unwrap()
                .y
        };
        assert_eq!(rebuilt_scroll_y, 0.0);
    }

    #[test]
    fn collection_tab_for_body_without_moons_is_rejected_without_mutation() {
        let loaded = LoadedCatalog::new(catalog());
        let io = loaded.index_of("io").unwrap();
        let mut state = LeftPanelUiState {
            selected_body_index: Some(io),
            page: ActivePanelPage::Info,
            dirty: false,
            scroll_y: 47.0,
            reset_scroll_on_rebuild: false,
            restore_focus: None,
            ..default()
        };
        let before = (
            state.selected_body_index,
            state.page,
            state.dirty,
            state.scroll_y,
            state.reset_scroll_on_rebuild,
        );
        let mut view_options = ViewOptionsState::default();
        let mut navigation = NavigationStack::root();
        sync_navigation_to_body(&loaded, io, &mut navigation);
        let before_navigation = navigation.clone();

        consume_left_panel_command(
            &SimCommand::SetLeftPanelTab(LeftPanelTab::Collection),
            Some(&loaded),
            &mut view_options,
            &mut state,
            &mut navigation,
        );

        assert_eq!(
            (
                state.selected_body_index,
                state.page,
                state.dirty,
                state.scroll_y,
                state.reset_scroll_on_rebuild,
            ),
            before
        );
        assert_eq!(navigation, before_navigation);
    }

    #[test]
    fn absent_requested_panel_action_resolves_to_the_explicit_fallback() {
        let mut world = World::new();
        let info = world
            .spawn(PanelAction::SelectPage(ActivePanelPage::Info))
            .id();
        assert_eq!(
            panel_focus_entity(
                &mut world,
                PanelAction::TravelTo(42),
                PanelAction::SelectPage(ActivePanelPage::Info),
            ),
            Some(info)
        );
    }

    fn visible_text_contains(app: &mut App, needle: &str) -> bool {
        let mut text = app.world_mut().query::<&Text>();
        text.iter(app.world()).any(|text| text.contains(needle))
    }

    #[test]
    fn moon_collection_counts_match_catalog_topology_for_every_parent() {
        let catalog = catalog();
        let collections = moon_collections(&catalog).unwrap();
        let expected_parent_count = catalog
            .bodies
            .iter()
            .filter(|parent| {
                catalog.bodies.iter().any(|body| {
                    body.category == Category::Moon
                        && body.parent.as_deref() == Some(parent.id.as_str())
                })
            })
            .count();
        assert_eq!(collections.len(), expected_parent_count);

        for collection in collections {
            let actual: Vec<_> = catalog
                .bodies
                .iter()
                .filter(|body| {
                    body.category == Category::Moon
                        && body.parent.as_deref() == Some(collection.parent.id.as_str())
                })
                .map(|body| body.id.as_str())
                .collect();
            let modeled: Vec<_> = collection
                .children
                .iter()
                .map(|body| body.id.as_str())
                .collect();
            assert_eq!(modeled, actual);
            assert_eq!(collection.count(), actual.len());
            assert_eq!(
                collection.label,
                format!("Moons of {} ({})", collection.parent.name, actual.len())
            );
        }
    }

    #[test]
    fn fifty_x_size_changes_only_render_scale_not_propagation_or_picking_truth() {
        let catalog = catalog();
        let earth = catalog
            .bodies
            .iter()
            .position(|body| body.id == "earth")
            .unwrap();
        let states = propagate_catalog(&catalog, 0.0).unwrap();
        let truth_before = states.0.clone();
        let true_radius_units = catalog.bodies[earth].radius_km / KM_PER_RENDER_UNIT;

        assert_eq!(
            rendered_body_radius_units(catalog.bodies[earth].radius_km, BodySizeScale::X50),
            (true_radius_units * 50.0) as f32
        );
        assert_eq!(states.0, truth_before);

        let projection = Projection::Perspective(PerspectiveProjection::default());
        let pick_radius = inflated_pick_radius(true_radius_units, 100.0, &projection, 720.0);
        assert_eq!(pick_radius, true_radius_units);
        let ray_inside_exaggerated_visual = [10.0, 0.0, -100.0];
        assert!(10.0 < true_radius_units * 50.0);
        assert_eq!(
            ray_sphere_hit_distance(
                [0.0; 3],
                ray_inside_exaggerated_visual,
                [0.0, 0.0, -100.0],
                pick_radius,
            ),
            None,
            "the ×50 visual silhouette must not expand picking truth"
        );
    }

    #[test]
    fn major_mode_hides_only_unflagged_moons_and_all_restores_them() {
        let catalog = catalog();
        let io = catalog
            .bodies
            .iter()
            .position(|body| body.id == "io")
            .unwrap();
        let himalia = catalog
            .bodies
            .iter()
            .position(|body| body.id == "himalia")
            .unwrap();
        let jupiter = catalog
            .bodies
            .iter()
            .position(|body| body.id == "jupiter")
            .unwrap();
        assert!(catalog.bodies[io].is_major_moon);
        assert!(!catalog.bodies[himalia].is_major_moon);

        let mut settings = ViewOptionsState::default();
        settings.set_moon_visibility("jupiter", MoonVisibilityMode::Major);
        let mut app = App::new();
        app.insert_resource(LoadedCatalog::new(catalog))
            .insert_resource(settings)
            .insert_resource(CameraController::new(jupiter, [0.0; 3], 10_000.0))
            .add_systems(Update, apply_view_options);
        let io_entity = app
            .world_mut()
            .spawn((
                BodyVisual { index: io },
                Transform::default(),
                Visibility::Visible,
            ))
            .id();
        let himalia_entity = app
            .world_mut()
            .spawn((
                BodyVisual { index: himalia },
                Transform::default(),
                Visibility::Visible,
            ))
            .id();
        app.update();

        assert_eq!(
            app.world().entity(io_entity).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(himalia_entity).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );

        app.world_mut()
            .resource_mut::<ViewOptionsState>()
            .set_moon_visibility("jupiter", MoonVisibilityMode::All);
        app.update();
        assert_eq!(
            app.world().entity(himalia_entity).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
    }

    #[test]
    fn initial_full_system_view_hides_every_moon_sphere() {
        let catalog = catalog();
        let loaded = LoadedCatalog::new(catalog);
        let sun = loaded.index_of("sun").unwrap();
        let earth = loaded.index_of("earth").unwrap();
        let moon_indices = loaded
            .catalog
            .bodies
            .iter()
            .enumerate()
            .filter_map(|(index, body)| (body.category == Category::Moon).then_some(index))
            .collect::<Vec<_>>();

        let mut app = App::new();
        app.insert_resource(loaded)
            .insert_resource(ViewOptionsState::default())
            .insert_resource(LayerState::default())
            .insert_resource(CameraController::new(sun, [0.0; 3], 10_000.0))
            .add_systems(Update, apply_view_options);
        let moon_entities = moon_indices
            .into_iter()
            .map(|index| {
                app.world_mut()
                    .spawn((BodyVisual { index }, Visibility::Visible))
                    .id()
            })
            .collect::<Vec<_>>();
        let earth_entity = app
            .world_mut()
            .spawn((BodyVisual { index: earth }, Visibility::Visible))
            .id();
        app.update();

        assert!(moon_entities.iter().all(|entity| {
            app.world().entity(*entity).get::<Visibility>() == Some(&Visibility::Hidden)
        }));
        assert_eq!(
            app.world().entity(earth_entity).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
    }

    #[test]
    fn focusing_a_moon_reveals_only_its_parent_system_and_moons_off_hides_it() {
        let catalog = catalog();
        let loaded = LoadedCatalog::new(catalog);
        let io = loaded.index_of("io").unwrap();
        let himalia = loaded.index_of("himalia").unwrap();
        let nereid = loaded.index_of("nereid").unwrap();

        let mut app = App::new();
        app.insert_resource(loaded)
            .insert_resource(ViewOptionsState::default())
            .insert_resource(LayerState::default())
            .insert_resource(CameraController::new(io, [0.0; 3], 10_000.0))
            .add_systems(Update, apply_view_options);
        let entities = [io, himalia, nereid].map(|index| {
            app.world_mut()
                .spawn((BodyVisual { index }, Visibility::Visible))
                .id()
        });
        app.update();

        assert_eq!(
            app.world().entity(entities[0]).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(entities[1]).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(entities[2]).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );

        app.world_mut()
            .resource_mut::<LayerState>()
            .set_visible(crate::LayerId::Moons, false);
        app.update();
        assert!(entities.iter().all(|entity| {
            app.world().entity(*entity).get::<Visibility>() == Some(&Visibility::Hidden)
        }));
    }

    #[test]
    fn search_travel_to_triton_reveals_the_neptune_system_on_arrival() {
        let catalog = catalog();
        let indices = catalog.id_index();
        let triton = *indices.get("triton").unwrap();
        let proteus = *indices.get("proteus").unwrap();
        let io = *indices.get("io").unwrap();
        let mut simulation = HeadlessSimulation::new(&catalog).unwrap();
        simulation
            .step(
                1.0 / 60.0,
                &[SimCommand::TravelToBody("triton".into())],
                None,
            )
            .unwrap();
        assert_ne!(simulation.camera().focus_body_index(), triton);
        for _ in 0..75 {
            simulation.step(1.0 / 60.0, &[], None).unwrap();
        }
        assert_eq!(simulation.camera().focus_body_index(), triton);

        let mut app = App::new();
        app.insert_resource(LoadedCatalog::new(catalog))
            .insert_resource(ViewOptionsState::default())
            .insert_resource(LayerState::default())
            .insert_resource(simulation.camera().clone())
            .add_systems(Update, apply_view_options);
        let entities = [triton, proteus, io].map(|index| {
            app.world_mut()
                .spawn((BodyVisual { index }, Visibility::Hidden))
                .id()
        });
        app.update();

        assert_eq!(
            app.world().entity(entities[0]).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(entities[1]).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(entities[2]).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn category_layers_hide_only_their_body_spheres_within_one_update() {
        let catalog = catalog();
        let earth = catalog
            .bodies
            .iter()
            .position(|body| body.id == "earth")
            .unwrap();
        let io = catalog
            .bodies
            .iter()
            .position(|body| body.id == "io")
            .unwrap();
        let jupiter = catalog
            .bodies
            .iter()
            .position(|body| body.id == "jupiter")
            .unwrap();
        let mut layers = LayerState::default();
        layers.set_visible(crate::LayerId::Planets, false);

        let mut app = App::new();
        app.insert_resource(LoadedCatalog::new(catalog))
            .insert_resource(ViewOptionsState::default())
            .insert_resource(layers)
            .insert_resource(CameraController::new(jupiter, [0.0; 3], 10_000.0))
            .add_systems(Update, apply_view_options);
        let earth_entity = app
            .world_mut()
            .spawn((
                BodyVisual { index: earth },
                Transform::default(),
                Visibility::Visible,
            ))
            .id();
        let io_entity = app
            .world_mut()
            .spawn((
                BodyVisual { index: io },
                Transform::default(),
                Visibility::Visible,
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().entity(earth_entity).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().entity(io_entity).get::<Visibility>(),
            Some(&Visibility::Visible)
        );

        {
            let mut layers = app.world_mut().resource_mut::<LayerState>();
            layers.set_visible(crate::LayerId::Planets, true);
            layers.set_visible(crate::LayerId::Moons, false);
        }
        app.update();
        assert_eq!(
            app.world().entity(earth_entity).get::<Visibility>(),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().entity(io_entity).get::<Visibility>(),
            Some(&Visibility::Hidden)
        );
    }

    #[test]
    fn view_options_snapshot_round_trips_for_wp14_without_loss() {
        let mut state = ViewOptionsState::default();
        state.set_panel_collapsed(true);
        state.set_body_size(BodySizeScale::X10);
        state.set_moon_visibility("jupiter", MoonVisibilityMode::All);
        state.set_local_orbit_visible("io", false);
        let snapshot = state.persistence_snapshot();

        let mut restored = ViewOptionsState::default();
        restored.restore_persistence_snapshot(snapshot.clone());
        assert_eq!(restored.persistence_snapshot(), snapshot);
    }

    #[test]
    fn returning_view_options_to_effective_defaults_removes_redundant_overrides() {
        let mut state = ViewOptionsState::default();
        assert!(state.set_moon_visibility("jupiter", MoonVisibilityMode::Major));
        assert!(state.set_moon_visibility("jupiter", MoonVisibilityMode::All));
        assert!(state.set_local_orbit_visible("io", false));
        assert!(state.set_local_orbit_visible("io", true));

        assert_eq!(state, ViewOptionsState::default());
        assert!(!state.set_moon_visibility("jupiter", MoonVisibilityMode::All));
        assert!(!state.set_local_orbit_visible("io", true));
    }

    #[test]
    fn left_panel_scroll_clamps_line_and_pixel_input() {
        assert_eq!(
            next_panel_scroll_y(100.0, -2.0, MouseScrollUnit::Line, 1_000.0, 600.0, 1.0),
            156.0
        );
        assert_eq!(
            next_panel_scroll_y(390.0, -50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0,),
            400.0
        );
        assert_eq!(
            next_panel_scroll_y(10.0, 50.0, MouseScrollUnit::Pixel, 1_000.0, 600.0, 1.0),
            0.0
        );
    }

    #[test]
    fn collection_action_navigates_to_the_catalog_derived_moons_page() {
        let loaded = LoadedCatalog::new(catalog());
        let jupiter = loaded.index_of("jupiter").unwrap();
        let mut app = App::new();
        app.insert_resource(loaded)
            .insert_resource(LeftPanelUiState {
                selected_body_index: Some(jupiter),
                page: ActivePanelPage::Info,
                dirty: false,
                scroll_y: 0.0,
                reset_scroll_on_rebuild: false,
                restore_focus: None,
                ..default()
            })
            .insert_resource(ViewOptionsState::default())
            .insert_resource(NavigationStack::root())
            .init_resource::<InputFocus>()
            .insert_resource(SimCommandQueue::default());
        let button = app
            .world_mut()
            .spawn(PanelAction::SelectPage(ActivePanelPage::Collection))
            .observe(activate_panel_action)
            .id();

        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(button, FocusCause::Navigated);
        app.world_mut().trigger(Activate { entity: button });
        assert_eq!(
            app.world().resource::<LeftPanelUiState>().restore_focus,
            Some(PanelAction::SelectPage(ActivePanelPage::Collection))
        );

        let command = app
            .world_mut()
            .resource_mut::<SimCommandQueue>()
            .drain()
            .next()
            .expect("collection tab command");
        assert_eq!(
            command,
            SimCommand::SetLeftPanelTab(LeftPanelTab::Collection)
        );
        let loaded = app.world_mut().remove_resource::<LoadedCatalog>().unwrap();
        let mut state = app
            .world_mut()
            .remove_resource::<LeftPanelUiState>()
            .unwrap();
        let mut settings = app
            .world_mut()
            .remove_resource::<ViewOptionsState>()
            .unwrap();
        let mut navigation = app
            .world_mut()
            .remove_resource::<NavigationStack>()
            .unwrap();
        consume_left_panel_command(
            &command,
            Some(&loaded),
            &mut settings,
            &mut state,
            &mut navigation,
        );
        app.insert_resource(loaded)
            .insert_resource(state)
            .insert_resource(settings)
            .insert_resource(navigation);
        assert_eq!(
            app.world().resource::<LeftPanelUiState>().page,
            ActivePanelPage::Collection
        );
        assert_eq!(
            app.world().resource::<NavigationStack>().label(),
            "Solar System › Jupiter › Moons"
        );
    }
}
