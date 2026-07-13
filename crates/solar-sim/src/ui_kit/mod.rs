//! WP7 reusable HUD kit — Rev C §§8.4, 9, and 9.1.
//!
//! Public scene-function signatures form the compatibility boundary for later
//! work packages. Implementations remain free to use BSN, classic spawn, or a
//! future Feathers replacement without changing their call sites.

#[cfg(debug_assertions)]
mod gallery;
mod hud;
mod navigation;
mod theme;
mod widgets;

#[cfg(debug_assertions)]
pub use gallery::{WidgetGalleryCell, WidgetGalleryRoot};
pub use hud::{top_bar, BreadcrumbText, SearchPlaceholder, TopBarRoot, TOP_BAR_HEIGHT_PX};
pub use navigation::{NavigationItem, NavigationStack, BREADCRUMB_SEPARATOR};
pub use theme::{UiColorToken, UiColors, UiSpacing, UiTheme, UiTypeScale};
pub use widgets::{
    checkbox_row, chip, panel, section_header, slider, tab_bar, toast, WidgetKind, WidgetRoot,
    WidgetSpec, WidgetVisualState, INTER_FONT_ASSET,
};

use bevy::prelude::*;

pub struct UiKitPlugin;

impl Plugin for UiKitPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTheme>()
            .init_resource::<NavigationStack>()
            .add_systems(Startup, hud::spawn_top_bar)
            .add_systems(Update, hud::update_breadcrumb);

        #[cfg(debug_assertions)]
        app.add_systems(Startup, gallery::spawn_widget_gallery);
    }
}
