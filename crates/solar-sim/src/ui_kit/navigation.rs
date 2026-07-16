//! WP7 breadcrumb navigation model — Rev C §9.1.
//!
//! The breadcrumb is the navigation stack, not a derived decoration. Stable
//! route ids are retained beside semantic destinations and display labels so
//! command/replay handling never has to infer a page from presentation text.

use bevy::prelude::Resource;

pub const BREADCRUMB_SEPARATOR: &str = " › ";
pub const ROOT_NAVIGATION_ID: &str = "solar_system";
const COLLECTION_ROUTE_SUFFIX: &str = "_moons";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NavigationDestination {
    Root,
    Body { body_id: String },
    Collection { parent_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationItem {
    pub id: String,
    pub label: String,
    pub destination: NavigationDestination,
}

impl NavigationItem {
    fn root() -> Self {
        Self {
            id: ROOT_NAVIGATION_ID.into(),
            label: "Solar System".into(),
            destination: NavigationDestination::Root,
        }
    }

    pub fn body(id: impl Into<String>, label: impl Into<String>) -> Self {
        let body_id = id.into();
        Self {
            id: body_id.clone(),
            label: label.into(),
            destination: NavigationDestination::Body { body_id },
        }
    }

    pub fn collection(parent_id: impl Into<String>, label: impl Into<String>) -> Self {
        let parent_id = parent_id.into();
        Self {
            id: format!("{parent_id}{COLLECTION_ROUTE_SUFFIX}"),
            label: label.into(),
            destination: NavigationDestination::Collection { parent_id },
        }
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct NavigationStack {
    items: Vec<NavigationItem>,
}

impl NavigationStack {
    pub fn root() -> Self {
        Self {
            items: vec![NavigationItem::root()],
        }
    }

    pub fn items(&self) -> &[NavigationItem] {
        &self.items
    }

    /// Pushes a body destination. Retained for the public WP7 call-site
    /// contract; collection pages must use [`Self::push_collection`].
    pub fn push(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.items.push(NavigationItem::body(id, label));
    }

    pub fn push_collection(&mut self, parent_id: impl Into<String>, label: impl Into<String>) {
        self.items
            .push(NavigationItem::collection(parent_id, label));
    }

    /// Pops one level but never removes the product root.
    pub fn pop(&mut self) -> Option<NavigationItem> {
        (self.items.len() > 1).then(|| self.items.pop()).flatten()
    }

    /// Keeps `len` leading levels. The root is always retained.
    pub fn truncate(&mut self, len: usize) {
        self.items.truncate(len.max(1));
    }

    pub fn destination_at(&self, depth: usize, target_id: &str) -> Option<&NavigationDestination> {
        self.items
            .get(depth)
            .filter(|item| item.id == target_id)
            .map(|item| &item.destination)
    }

    pub fn label(&self) -> String {
        self.items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>()
            .join(BREADCRUMB_SEPARATOR)
    }
}

impl Default for NavigationStack {
    fn default() -> Self {
        Self::root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_breadcrumb_push_pop_and_truncate_sequence_is_exact() {
        let mut navigation = NavigationStack::root();
        assert_eq!(navigation.label(), "Solar System");

        navigation.push("jupiter", "Jupiter");
        navigation.push_collection("jupiter", "Moons");
        assert_eq!(navigation.label(), "Solar System › Jupiter › Moons");
        assert_eq!(
            navigation.destination_at(2, "jupiter_moons"),
            Some(&NavigationDestination::Collection {
                parent_id: "jupiter".into(),
            })
        );

        assert_eq!(navigation.pop().unwrap().id, "jupiter_moons");
        assert_eq!(navigation.label(), "Solar System › Jupiter");

        navigation.push("io", "Io");
        navigation.push("io_surface", "Surface");
        navigation.truncate(2);
        assert_eq!(navigation.label(), "Solar System › Jupiter");

        navigation.truncate(0);
        assert_eq!(navigation.label(), "Solar System");
        assert!(navigation.pop().is_none());
    }

    #[test]
    fn destination_lookup_requires_matching_depth_and_stable_route_id() {
        let mut navigation = NavigationStack::root();
        navigation.push("jupiter", "Jupiter");
        navigation.push_collection("jupiter", "Moons");

        assert_eq!(
            navigation.destination_at(1, "jupiter"),
            Some(&NavigationDestination::Body {
                body_id: "jupiter".into(),
            })
        );
        assert!(navigation.destination_at(0, "jupiter").is_none());
        assert!(navigation.destination_at(1, "jupiter_moons").is_none());
        assert!(navigation.destination_at(99, "jupiter").is_none());
    }
}
