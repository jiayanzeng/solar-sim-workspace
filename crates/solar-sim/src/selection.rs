//! WP6/WP9 — render-only selection accent shared by bodies and orbit paths.
//!
//! Canonical selection remains `CameraController` state reduced from
//! `SimCommand`. This resource mirrors only the selected catalog index so
//! retained render systems can react once without entering replay hashes or
//! inventing a second selection model (Rev C §§3.4, 10.3).

use crate::{CameraController, LoadedCatalog, SimulationSet};
use bevy::prelude::*;

pub(crate) const SELECTION_ACCENT_RGB: [u8; 3] = [76, 211, 255];
pub(crate) const SELECTION_BODY_BLEND: f32 = 0.45;

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionAccent {
    selected_body_index: Option<usize>,
}

impl FromWorld for SelectionAccent {
    fn from_world(world: &mut World) -> Self {
        Self {
            selected_body_index: world
                .get_resource::<CameraController>()
                .map(CameraController::selected_body_index),
        }
    }
}

impl SelectionAccent {
    pub(crate) const fn accents_body(self, body_index: usize) -> bool {
        matches!(self.selected_body_index, Some(selected) if selected == body_index)
    }

    pub(crate) fn accents_orbit(self, loaded: &LoadedCatalog, body_index: usize) -> bool {
        let Some(selected) = self.selected_body_index else {
            return false;
        };
        if selected == body_index {
            return true;
        }
        loaded
            .catalog
            .bodies
            .get(body_index)
            .and_then(|body| body.parent.as_deref())
            .and_then(|parent_id| loaded.index_of(parent_id))
            == Some(selected)
    }

    #[cfg(test)]
    pub(crate) const fn for_selected(body_index: usize) -> Self {
        Self {
            selected_body_index: Some(body_index),
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SelectionAccentSet;

pub(crate) fn install_selection_accent(app: &mut App) {
    app.init_resource::<SelectionAccent>().add_systems(
        Update,
        sync_selection_accent
            .in_set(SelectionAccentSet)
            .in_set(SimulationSet::Render),
    );
}

fn sync_selection_accent(camera: Res<CameraController>, mut selection: ResMut<SelectionAccent>) {
    let selected_body_index = Some(camera.selected_body_index());
    if selection.selected_body_index == selected_body_index {
        return;
    }
    selection.bypass_change_detection().selected_body_index = selected_body_index;
    selection.set_changed();
}

pub(crate) fn blend_selection_rgb(base: [f32; 3], selected: bool) -> [f32; 3] {
    if !selected {
        return base;
    }
    let accent = SELECTION_ACCENT_RGB.map(|channel| f32::from(channel) / 255.0);
    std::array::from_fn(|index| base[index] + (accent[index] - base[index]) * SELECTION_BODY_BLEND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_catalog_text;

    const REAL_CATALOG: &str = include_str!("../../../assets/catalog.ron");

    #[test]
    fn selected_parent_accents_its_body_own_orbit_and_catalog_children_only() {
        let loaded = LoadedCatalog::new(load_catalog_text(REAL_CATALOG).unwrap());
        let jupiter = loaded.index_of("jupiter").unwrap();
        let io = loaded.index_of("io").unwrap();
        let europa = loaded.index_of("europa").unwrap();
        let saturn = loaded.index_of("saturn").unwrap();
        let selection = SelectionAccent::for_selected(jupiter);

        assert!(selection.accents_body(jupiter));
        assert!(!selection.accents_body(io));
        assert!(selection.accents_orbit(&loaded, jupiter));
        assert!(selection.accents_orbit(&loaded, io));
        assert!(selection.accents_orbit(&loaded, europa));
        assert!(!selection.accents_orbit(&loaded, saturn));

        let moon = SelectionAccent::for_selected(io);
        assert!(moon.accents_body(io));
        assert!(moon.accents_orbit(&loaded, io));
        assert!(!moon.accents_orbit(&loaded, europa));
    }

    #[test]
    fn selection_blend_restores_the_exact_base_rgb_when_not_selected() {
        let base = [0.2, 0.4, 0.7];
        assert_eq!(blend_selection_rgb(base, false), base);
        let selected = blend_selection_rgb(base, true);
        assert_ne!(selected, base);
        assert!(selected[1] > base[1]);
        assert!(selected[2] > base[2]);
    }

    #[test]
    fn camera_pose_changes_do_not_advertise_a_false_selection_change() {
        #[derive(Resource, Default)]
        struct SelectionWrites(usize);

        fn count_selection_writes(
            selection: Res<SelectionAccent>,
            mut writes: ResMut<SelectionWrites>,
        ) {
            if selection.is_changed() {
                writes.0 += 1;
            }
        }

        let loaded = LoadedCatalog::new(load_catalog_text(REAL_CATALOG).unwrap());
        let sun = loaded.index_of("sun").unwrap();
        let mut app = App::new();
        app.insert_resource(CameraController::new(sun, [0.0; 3], 1.0e6))
            .init_resource::<SelectionWrites>();
        install_selection_accent(&mut app);
        app.add_systems(Update, count_selection_writes.after(SelectionAccentSet));
        app.update();
        app.world_mut().resource_mut::<SelectionWrites>().0 = 0;

        app.world_mut()
            .resource_mut::<CameraController>()
            .set_initial_pose(0.7, -0.2, 2.0e6);
        app.update();

        assert_eq!(app.world().resource::<SelectionWrites>().0, 0);
    }
}
