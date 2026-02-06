//! Selection region registry.
//!
//! The region registry tracks clickable/selectable areas on the screen.
/// It's rebuilt every frame following the immediate-mode pattern.
/// Widgets register their screen `Rect` during `draw()`.
use ratatui::layout::Rect;

/// Unique identifier for a selection region.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegionId(pub String);

impl From<&str> for RegionId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for RegionId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A selectable region on the screen.
#[derive(Debug, Clone)]
pub struct SelectionRegion {
    pub id: RegionId,
    pub rect: Rect,
    pub z_order: u16,
}

impl SelectionRegion {
    pub fn new(id: RegionId, rect: Rect, z_order: u16) -> Self {
        Self { id, rect, z_order }
    }

    /// Check if a point (col, row) is inside this region.
    pub fn contains(&self, col: u16, row: u16) -> bool {
        let col = col as u32;
        let row = row as u32;
        let x = self.rect.x as u32;
        let y = self.rect.y as u32;
        let width = self.rect.width as u32;
        let height = self.rect.height as u32;
        col >= x && col < x + width && row >= y && row < y + height
    }
}

/// Registry of selection regions, rebuilt each frame.
#[derive(Debug, Default)]
pub struct RegionRegistry {
    regions: Vec<SelectionRegion>,
}

impl RegionRegistry {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Clear all registered regions. Called at the start of each frame.
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Register a new selection region.
    pub fn register(&mut self, id: RegionId, rect: Rect, z_order: u16) {
        self.regions.push(SelectionRegion::new(id, rect, z_order));
    }

    /// Find the topmost region (highest z_order) at the given coordinates.
    pub fn region_at(&self, col: u16, row: u16) -> Option<&SelectionRegion> {
        self.regions
            .iter()
            .filter(|r| r.contains(col, row))
            .max_by_key(|r| r.z_order)
    }

    /// Find a region by its ID.
    pub fn region_by_id(&self, id: &RegionId) -> Option<&SelectionRegion> {
        self.regions.iter().find(|r| r.id == *id)
    }

    /// Returns the number of registered regions.
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Returns true if no regions are registered.
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Iterate over all registered regions.
    pub fn iter(&self) -> impl Iterator<Item = &SelectionRegion> {
        self.regions.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_id_from_str() {
        let id: RegionId = "test_region".into();
        assert_eq!(id.0, "test_region");
    }

    #[test]
    fn test_selection_region_contains() {
        let region = SelectionRegion::new(
            "test".into(),
            Rect::new(10, 5, 20, 10), // x=10, y=5, width=20, height=10
            0,
        );

        // Inside
        assert!(region.contains(15, 7));
        assert!(region.contains(10, 5)); // Top-left corner
        assert!(region.contains(29, 14)); // Bottom-right (inclusive)

        // Outside
        assert!(!region.contains(9, 7)); // Left of region
        assert!(!region.contains(30, 7)); // Right of region
        assert!(!region.contains(15, 4)); // Above region
        assert!(!region.contains(15, 15)); // Below region
    }

    #[test]
    fn test_region_registry_clear() {
        let mut registry = RegionRegistry::new();
        registry.register("test".into(), Rect::new(0, 0, 10, 10), 0);
        assert_eq!(registry.len(), 1);

        registry.clear();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_region_registry_region_at() {
        let mut registry = RegionRegistry::new();
        registry.register("background".into(), Rect::new(0, 0, 100, 100), 0);
        registry.register("modal".into(), Rect::new(30, 20, 40, 30), 100);
        registry.register("sidebar".into(), Rect::new(0, 0, 20, 100), 10);

        // Point only in background
        let region = registry.region_at(50, 50);
        assert!(region.is_some());
        assert_eq!(region.unwrap().id.0, "background");

        // Point in sidebar (z=10)
        let region = registry.region_at(10, 50);
        assert!(region.is_some());
        assert_eq!(region.unwrap().id.0, "sidebar");

        // Point in modal (z=100, highest)
        let region = registry.region_at(40, 30);
        assert!(region.is_some());
        assert_eq!(region.unwrap().id.0, "modal");

        // Point outside all regions
        let region = registry.region_at(200, 200);
        assert!(region.is_none());
    }

    #[test]
    fn test_region_registry_region_by_id() {
        let mut registry = RegionRegistry::new();
        registry.register("test".into(), Rect::new(0, 0, 10, 10), 0);

        let found = registry.region_by_id(&"test".into());
        assert!(found.is_some());

        let not_found = registry.region_by_id(&"missing".into());
        assert!(not_found.is_none());
    }
}
