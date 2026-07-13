//! WP7 breadcrumb navigation model — Rev C §9.1.
//!
//! The breadcrumb is the navigation stack, not a derived decoration. Stable
//! ids are retained beside display labels so later collection/search packages
//! can navigate without coupling commands to presentation strings.

use bevy::prelude::Resource;

pub const BREADCRUMB_SEPARATOR: &str = " › ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationItem {
    pub id: String,
    pub label: String,
}

impl NavigationItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
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
            items: vec![NavigationItem::new("solar_system", "Solar System")],
        }
    }

    pub fn items(&self) -> &[NavigationItem] {
        &self.items
    }

    pub fn push(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.items.push(NavigationItem::new(id, label));
    }

    /// Pops one level but never removes the product root.
    pub fn pop(&mut self) -> Option<NavigationItem> {
        (self.items.len() > 1).then(|| self.items.pop()).flatten()
    }

    /// Keeps `len` leading levels. The root is always retained.
    pub fn truncate(&mut self, len: usize) {
        self.items.truncate(len.max(1));
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
        navigation.push("jupiter_moons", "Moons");
        assert_eq!(navigation.label(), "Solar System › Jupiter › Moons");

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
}
