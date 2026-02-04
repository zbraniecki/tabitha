//! Focus management for the TUI framework.
//!
//! This module provides focus navigation and event propagation control.
//!
//! # Hierarchical Focus Model
//!
//! The focus system supports hierarchical navigation where elements can have
//! parent-child relationships:
//!
//! - Each focusable element has a unique string ID
//! - Elements can be nested in parent-child relationships
//! - Navigation stays within the current level by default
//! - `focus_into()` enters a child container
//! - `focus_out()` returns to the parent level
//!
//! # Example
//!
//! ```ignore
//! // Register hierarchical structure
//! focus.register("form");
//! focus.register_child("form", "username");
//! focus.register_child("form", "password");
//! focus.register_child("form", "submit");
//!
//! // Navigate
//! focus.focus_into("form");    // Enter form, focuses first child
//! focus.next_sibling();        // Next field within form
//! focus.focus_out();           // Exit to parent
//! ```

use std::collections::HashMap;

/// Result of event handling that controls propagation.
///
/// Used as the return type for `handle_event` methods to indicate
/// whether the event was consumed and whether propagation should continue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventResult {
    /// Event was not handled, continue propagation (bubble to parent).
    #[default]
    Unhandled,
    /// Event was handled, stop propagation.
    Handled,
    /// Stop propagation without marking as handled.
    StopPropagation,
}

impl EventResult {
    /// Check if the event was handled.
    #[inline]
    pub fn is_handled(&self) -> bool {
        matches!(self, EventResult::Handled)
    }

    /// Check if propagation should continue.
    #[inline]
    pub fn should_propagate(&self) -> bool {
        matches!(self, EventResult::Unhandled)
    }
}

impl From<bool> for EventResult {
    fn from(handled: bool) -> Self {
        if handled {
            EventResult::Handled
        } else {
            EventResult::Unhandled
        }
    }
}

impl From<EventResult> for bool {
    fn from(result: EventResult) -> Self {
        result.is_handled()
    }
}

/// A node in the focus tree.
#[derive(Debug, Clone)]
struct FocusNode {
    /// Unique identifier for this node.
    /// Stored for debugging purposes; the ID is also the HashMap key.
    #[allow(dead_code)]
    id: String,
    /// Parent node ID, if any.
    parent: Option<String>,
    /// Child node IDs in order.
    children: Vec<String>,
}

impl FocusNode {
    fn new(id: String) -> Self {
        Self {
            id,
            parent: None,
            children: Vec::new(),
        }
    }
}

/// Manages focus state and navigation with hierarchical support.
///
/// The `FocusManager` tracks which UI elements are focusable and which
/// one currently has focus. It supports both flat and hierarchical
/// navigation models.
pub struct FocusManager {
    /// All nodes in the focus tree.
    nodes: HashMap<String, FocusNode>,
    /// Root-level node IDs (no parent).
    root_nodes: Vec<String>,
    /// Current focus path (stack of node IDs from root to leaf).
    focus_path: Vec<String>,
}

impl FocusManager {
    /// Create a new empty focus manager.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
            focus_path: Vec::new(),
        }
    }

    /// Register a focusable element as a root-level node.
    ///
    /// If the element is already registered, this does nothing.
    pub fn register(&mut self, id: &str) {
        if self.nodes.contains_key(id) {
            return;
        }

        let node = FocusNode::new(id.to_string());
        self.nodes.insert(id.to_string(), node);
        self.root_nodes.push(id.to_string());
    }

    /// Register a child node under a parent.
    ///
    /// Creates the parent if it doesn't exist. If the child is already
    /// registered elsewhere, it is moved to the new parent.
    pub fn register_child(&mut self, parent_id: &str, child_id: &str) {
        // Ensure parent exists
        if !self.nodes.contains_key(parent_id) {
            self.register(parent_id);
        }

        // Check if child already exists
        if let Some(existing_parent) = self.nodes.get(child_id).and_then(|n| n.parent.clone()) {
            if existing_parent == parent_id {
                // Already registered under this parent
                return;
            }
            // Remove from old parent
            if let Some(parent) = self.nodes.get_mut(&existing_parent) {
                parent.children.retain(|id| id != child_id);
            }
        } else if self.root_nodes.contains(&child_id.to_string()) {
            // Remove from roots
            self.root_nodes.retain(|id| id != child_id);
        }

        // Create or update child node
        let child = self
            .nodes
            .entry(child_id.to_string())
            .or_insert_with(|| FocusNode::new(child_id.to_string()));
        child.parent = Some(parent_id.to_string());

        // Add to parent's children
        if let Some(parent) = self.nodes.get_mut(parent_id) {
            if !parent.children.contains(&child_id.to_string()) {
                parent.children.push(child_id.to_string());
            }
        }
    }

    /// Register multiple children under a parent.
    pub fn register_children(&mut self, parent_id: &str, children: &[&str]) {
        for child_id in children {
            self.register_child(parent_id, child_id);
        }
    }

    /// Unregister a focusable element and all its children.
    ///
    /// If the element or any of its children were focused, focus is cleared.
    pub fn unregister(&mut self, id: &str) {
        // Collect all descendants to remove
        let mut to_remove = vec![id.to_string()];
        if let Some(node) = self.nodes.get(id) {
            Self::collect_descendants(node, &self.nodes, &mut to_remove);
        }

        // Remove from parent's children list
        let parent_id = self.nodes.get(id).and_then(|n| n.parent.clone());
        if let Some(ref parent_id) = parent_id {
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                parent.children.retain(|child_id| child_id != id);
            }
        } else {
            // Remove from root nodes
            self.root_nodes.retain(|root_id| root_id != id);
        }

        // Remove all descendants
        for remove_id in &to_remove {
            self.nodes.remove(remove_id);
        }

        // Clear focus if focused element was removed
        if self
            .focus_path
            .iter()
            .any(|path_id| to_remove.contains(path_id))
        {
            self.focus_path.clear();
        }
    }

    fn collect_descendants(
        node: &FocusNode,
        nodes: &HashMap<String, FocusNode>,
        result: &mut Vec<String>,
    ) {
        for child_id in &node.children {
            result.push(child_id.clone());
            if let Some(child) = nodes.get(child_id) {
                Self::collect_descendants(child, nodes, result);
            }
        }
    }

    /// Get the ID of the currently focused element (leaf of focus path).
    pub fn focused_id(&self) -> Option<&str> {
        self.focus_path.last().map(|s| s.as_str())
    }

    /// Get the current focus path as a slice of node IDs.
    pub fn focus_path(&self) -> &[String] {
        &self.focus_path
    }

    /// Check if a specific element is currently focused.
    pub fn is_focused(&self, id: &str) -> bool {
        self.focused_id() == Some(id)
    }

    /// Check if an element is focused or is an ancestor of focused element.
    pub fn is_focused_or_within(&self, id: &str) -> bool {
        self.focus_path.contains(&id.to_string())
    }

    /// Check if a specific element is in the focus chain.
    ///
    /// This is an alias for `is_focused_or_within` for backward compatibility.
    pub fn is_in_focus_chain(&self, id: &str) -> bool {
        self.is_focused_or_within(id)
    }

    /// Set focus to a specific element by ID.
    ///
    /// Returns `true` if the element was found and focused.
    pub fn set_focus(&mut self, id: &str) -> bool {
        if !self.nodes.contains_key(id) {
            return false;
        }

        // Build path from target to root
        let mut path = vec![id.to_string()];
        let mut current = id.to_string();

        while let Some(node) = self.nodes.get(&current) {
            if let Some(ref parent_id) = node.parent {
                path.push(parent_id.clone());
                current = parent_id.clone();
            } else {
                break;
            }
        }

        // Reverse to get root-to-leaf order
        path.reverse();
        self.focus_path = path;
        true
    }

    /// Clear focus (no element is focused).
    pub fn clear_focus(&mut self) {
        self.focus_path.clear();
    }

    /// Navigate to the next sibling at the current level.
    ///
    /// Returns `true` if focus moved. Wraps around to the first element
    /// when at the last sibling.
    pub fn next_sibling(&mut self) -> bool {
        let current_id = match self.focused_id() {
            Some(id) => id.to_string(),
            None => return self.focus_first(),
        };

        let siblings = self.get_siblings(&current_id);
        if siblings.is_empty() {
            return false;
        }

        let current_index = siblings.iter().position(|id| id == &current_id);

        let next_idx = match current_index {
            Some(idx) => (idx + 1) % siblings.len(),
            None => 0,
        };

        let next_id = siblings[next_idx].clone();
        self.update_focus_leaf(&next_id);
        true
    }

    /// Navigate to the previous sibling at the current level.
    ///
    /// Returns `true` if focus moved. Wraps around to the last element
    /// when at the first sibling.
    pub fn prev_sibling(&mut self) -> bool {
        let current_id = match self.focused_id() {
            Some(id) => id.to_string(),
            None => return self.focus_first(),
        };

        let siblings = self.get_siblings(&current_id);
        if siblings.is_empty() {
            return false;
        }

        let current_index = siblings.iter().position(|id| id == &current_id);

        let prev_idx = match current_index {
            Some(0) => siblings.len() - 1,
            Some(idx) => idx - 1,
            None => siblings.len() - 1,
        };

        let prev_id = siblings[prev_idx].clone();
        self.update_focus_leaf(&prev_id);
        true
    }

    fn get_siblings(&self, id: &str) -> Vec<String> {
        if let Some(node) = self.nodes.get(id) {
            if let Some(ref parent_id) = node.parent {
                if let Some(parent) = self.nodes.get(parent_id) {
                    return parent.children.clone();
                }
            }
        }
        self.root_nodes.clone()
    }

    fn update_focus_leaf(&mut self, new_id: &str) {
        if !self.focus_path.is_empty() {
            self.focus_path.pop();
        }
        self.focus_path.push(new_id.to_string());
    }

    /// Focus the first focusable element.
    ///
    /// Returns `true` if an element was focused.
    pub fn focus_first(&mut self) -> bool {
        if let Some(first) = self.root_nodes.first().cloned() {
            self.set_focus(&first);
            true
        } else {
            false
        }
    }

    /// Enter a child container's focus scope.
    ///
    /// If the focused element has children, enters the first child.
    /// Returns `true` if focus moved into a child.
    pub fn focus_into(&mut self) -> bool {
        let current_id = match self.focused_id() {
            Some(id) => id.to_string(),
            None => return self.focus_first(),
        };

        if let Some(node) = self.nodes.get(&current_id) {
            if let Some(first_child) = node.children.first() {
                self.focus_path.push(first_child.clone());
                return true;
            }
        }
        false
    }

    /// Exit current focus scope to parent.
    ///
    /// Returns `true` if focus moved to parent.
    pub fn focus_out(&mut self) -> bool {
        if self.focus_path.len() > 1 {
            self.focus_path.pop();
            true
        } else {
            false
        }
    }

    /// Move focus to the next element (sibling navigation).
    ///
    /// This is an alias for `next_sibling()` for backward compatibility.
    pub fn focus_next(&mut self) -> bool {
        self.next_sibling()
    }

    /// Move focus to the previous element (sibling navigation).
    ///
    /// This is an alias for `prev_sibling()` for backward compatibility.
    pub fn focus_prev(&mut self) -> bool {
        self.prev_sibling()
    }

    /// Get the list of root-level focusable elements.
    pub fn root_nodes(&self) -> &[String] {
        &self.root_nodes
    }

    /// Get the children of a specific node.
    pub fn children(&self, id: &str) -> Option<&[String]> {
        self.nodes.get(id).map(|n| n.children.as_slice())
    }

    /// Get the parent of a specific node.
    pub fn parent(&self, id: &str) -> Option<&str> {
        self.nodes.get(id).and_then(|n| n.parent.as_deref())
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchical_focus() {
        let mut fm = FocusManager::new();

        // Set up hierarchy: form -> [username, password, submit]
        fm.register("form");
        fm.register_child("form", "username");
        fm.register_child("form", "password");
        fm.register_child("form", "submit");

        // Focus into form
        fm.set_focus("form");
        assert_eq!(fm.focused_id(), Some("form"));
        assert!(fm.is_focused("form"));

        // Enter child
        assert!(fm.focus_into());
        assert_eq!(fm.focused_id(), Some("username"));
        assert!(fm.is_focused_or_within("form"));

        // Navigate within form
        assert!(fm.next_sibling());
        assert_eq!(fm.focused_id(), Some("password"));
        assert!(fm.next_sibling());
        assert_eq!(fm.focused_id(), Some("submit"));
        assert!(fm.next_sibling()); // Wraps to first
        assert_eq!(fm.focused_id(), Some("username"));

        // Exit to parent
        assert!(fm.focus_out());
        assert_eq!(fm.focused_id(), Some("form"));
        assert!(!fm.focus_out()); // Already at root
    }

    #[test]
    fn test_unregister_removes_children() {
        let mut fm = FocusManager::new();
        fm.register("form");
        fm.register_child("form", "input");
        fm.register_child("input", "cursor");

        fm.set_focus("cursor");
        assert_eq!(fm.focused_id(), Some("cursor"));

        fm.unregister("form");
        assert!(fm.focused_id().is_none());
        assert!(!fm.nodes.contains_key("input"));
        assert!(!fm.nodes.contains_key("cursor"));
    }

    #[test]
    fn test_flat_navigation() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");

        fm.focus_next();
        assert_eq!(fm.focused_id(), Some("a"));

        fm.focus_next();
        assert_eq!(fm.focused_id(), Some("b"));

        fm.focus_prev();
        assert_eq!(fm.focused_id(), Some("a"));

        // Test backward wraparound
        fm.focus_prev(); // Wraps from a to c
        assert_eq!(fm.focused_id(), Some("c"));

        fm.focus_prev();
        assert_eq!(fm.focused_id(), Some("b"));

        // Test forward wraparound
        fm.set_focus("c");
        fm.focus_next(); // Wraps from c to a
        assert_eq!(fm.focused_id(), Some("a"));
    }
}
