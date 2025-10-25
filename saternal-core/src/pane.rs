use crate::terminal::Terminal;
use anyhow::Result;
use log::info;

/// Direction for splitting panes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// A single terminal pane
pub struct Pane {
    pub id: usize,
    pub terminal: Terminal,
    pub focused: bool,
}

impl Pane {
    pub fn new(id: usize, cols: usize, rows: usize, shell: Option<String>) -> Result<Self> {
        let terminal = Terminal::new(cols, rows, shell)?;
        Ok(Self {
            id,
            terminal,
            focused: false,
        })
    }

    pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
        self.terminal.resize(cols, rows)
    }
}

/// Node in the pane tree - either a leaf (single pane) or a split
pub enum PaneNode {
    Leaf {
        pane: Pane,
    },
    Split {
        direction: SplitDirection,
        children: Vec<PaneNode>,
        /// Ratio of space allocation between children (0.0-1.0)
        ratio: f32,
    },
}

impl PaneNode {
    /// Create a new leaf node
    pub fn new_leaf(id: usize, cols: usize, rows: usize, shell: Option<String>) -> Result<Self> {
        let pane = Pane::new(id, cols, rows, shell)?;
        Ok(PaneNode::Leaf { pane })
    }

    /// Split this node in the given direction
    pub fn split(
        &mut self,
        direction: SplitDirection,
        new_id: usize,
        cols: usize,
        rows: usize,
        shell: Option<String>,
    ) -> Result<()> {
        // Take ownership of self without constructing a dummy pane
        let old_node = std::mem::replace(
            self,
            PaneNode::Split {
                direction,
                children: Vec::new(),
                ratio: 0.5,
            },
        );

        // Create new pane
        let new_pane = Pane::new(new_id, cols, rows, shell)?;
        let new_node = PaneNode::Leaf { pane: new_pane };

        // Populate children with old and new nodes
        if let PaneNode::Split { children, .. } = self {
            children.push(old_node);
            children.push(new_node);
        }

        info!("Split pane in {:?} direction", direction);
        Ok(())
    }

    /// Get the focused pane (immutable), if any
    pub fn focused_pane(&self) -> Option<&Pane> {
        match self {
            PaneNode::Leaf { pane } if pane.focused => Some(pane),
            PaneNode::Leaf { .. } => None,
            PaneNode::Split { children, .. } => children
                .iter()
                .find_map(|child| child.focused_pane()),
        }
    }

    /// Get the focused pane (mutable), if any
    pub fn focused_pane_mut(&mut self) -> Option<&mut Pane> {
        match self {
            PaneNode::Leaf { pane } if pane.focused => Some(pane),
            PaneNode::Leaf { .. } => None,
            PaneNode::Split { children, .. } => children
                .iter_mut()
                .find_map(|child| child.focused_pane_mut()),
        }
    }

    /// Set focus on a specific pane by ID
    pub fn set_focus(&mut self, id: usize) -> bool {
        match self {
            PaneNode::Leaf { pane } => {
                if pane.id == id {
                    pane.focused = true;
                    true
                } else {
                    pane.focused = false;
                    false
                }
            }
            PaneNode::Split { children, .. } => {
                let mut found = false;
                for child in children {
                    if child.set_focus(id) {
                        found = true;
                    }
                }
                found
            }
        }
    }

    /// Get all pane IDs in the tree
    pub fn pane_ids(&self) -> Vec<usize> {
        match self {
            PaneNode::Leaf { pane } => vec![pane.id],
            PaneNode::Split { children, .. } => {
                children.iter().flat_map(|c| c.pane_ids()).collect()
            }
        }
    }

    /// Get all panes in the tree with their IDs
    pub fn all_panes(&self) -> Vec<(usize, &Pane)> {
        match self {
            PaneNode::Leaf { pane } => vec![(pane.id, pane)],
            PaneNode::Split { children, .. } => {
                children.iter().flat_map(|c| c.all_panes()).collect()
            }
        }
    }

    /// Get all panes in the tree mutably with their IDs
    pub fn all_panes_mut(&mut self) -> Vec<(usize, &mut Pane)> {
        match self {
            PaneNode::Leaf { pane } => vec![(pane.id, pane)],
            PaneNode::Split { children, .. } => {
                children.iter_mut().flat_map(|c| c.all_panes_mut()).collect()
            }
        }
    }

    /// Find a pane by ID
    pub fn find_pane(&self, id: usize) -> Option<&Pane> {
        match self {
            PaneNode::Leaf { pane } if pane.id == id => Some(pane),
            PaneNode::Leaf { .. } => None,
            PaneNode::Split { children, .. } => {
                children.iter().find_map(|child| child.find_pane(id))
            }
        }
    }

    /// Split the currently focused pane
    pub fn split_focused(
        &mut self,
        direction: SplitDirection,
        new_id: usize,
        shell: Option<String>,
    ) -> Result<bool> {
        match self {
            PaneNode::Leaf { pane } if pane.focused => {
                // Found the focused pane - split it
                let (cols, rows) = pane.terminal.dimensions();

                // Calculate split dimensions based on direction
                let (new_cols, new_rows) = match direction {
                    SplitDirection::Horizontal => (cols, rows / 2),
                    SplitDirection::Vertical => (cols / 2, rows),
                };

                // Split this pane
                self.split(direction, new_id, new_cols.max(1), new_rows.max(1), shell)?;

                // Set focus to new pane
                if let PaneNode::Split { children, .. } = self {
                    if let Some(child) = children.get_mut(0) {
                        child.clear_focus();
                    }
                    if let Some(PaneNode::Leaf { pane }) = children.get_mut(1) {
                        pane.focused = true;
                    }
                }

                Ok(true)
            }
            PaneNode::Leaf { .. } => Ok(false),
            PaneNode::Split { children, .. } => {
                // Recursively search children for focused pane
                for child in children.iter_mut() {
                    if child.split_focused(direction, new_id, shell.clone())? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// Clear focus from all panes in this subtree
    fn clear_focus(&mut self) {
        match self {
            PaneNode::Leaf { pane } => pane.focused = false,
            PaneNode::Split { children, .. } => {
                for child in children {
                    child.clear_focus();
                }
            }
        }
    }

    /// Move focus to next pane (circular)
    pub fn focus_next(&mut self) -> bool {
        let pane_ids = self.pane_ids();
        if pane_ids.is_empty() {
            return false;
        }

        if let Some(current_pane) = self.focused_pane() {
            let current_id = current_pane.id;
            if let Some(current_idx) = pane_ids.iter().position(|&id| id == current_id) {
                let next_idx = (current_idx + 1) % pane_ids.len();
                let next_id = pane_ids[next_idx];
                return self.set_focus(next_id);
            }
        }
        false
    }

    /// Move focus to previous pane (circular)
    pub fn focus_prev(&mut self) -> bool {
        let pane_ids = self.pane_ids();
        if pane_ids.is_empty() {
            return false;
        }

        if let Some(current_pane) = self.focused_pane() {
            let current_id = current_pane.id;
            if let Some(current_idx) = pane_ids.iter().position(|&id| id == current_id) {
                let prev_idx = if current_idx == 0 {
                    pane_ids.len() - 1
                } else {
                    current_idx - 1
                };
                let prev_id = pane_ids[prev_idx];
                return self.set_focus(prev_id);
            }
        }
        false
    }

    /// Close the focused pane and rebalance the tree
    pub fn close_focused(&mut self) -> Result<bool> {
        match self {
            PaneNode::Leaf { pane } if pane.focused => {
                // Can't close from leaf level - parent must handle
                Ok(true)
            }
            PaneNode::Leaf { .. } => Ok(false),
            PaneNode::Split { children, .. } => {
                // Check if a direct child is focused and should be closed
                for i in 0..children.len() {
                    if let PaneNode::Leaf { pane } = &children[i] {
                        if pane.focused && children.len() == 2 {
                            // Remove this child and replace split with the other child
                            let other_idx = 1 - i;
                            let mut other_child = children.remove(other_idx);
                            *self = other_child;
                            return Ok(true);
                        }
                    }

                    // Recursively check children
                    if children[i].close_focused()? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// Resize all panes in the tree to specified terminal dimensions (cols x rows)
    pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
        match self {
            PaneNode::Leaf { pane } => {
                pane.resize(cols.max(1), rows.max(1))?;
            }
            PaneNode::Split {
                direction,
                children,
                ratio,
            } => {
                match direction {
                    SplitDirection::Horizontal => {
                        // Split rows between panes
                        let rows1 = (rows as f32 * *ratio) as usize;
                        let rows2 = rows.saturating_sub(rows1);
                        if let Some(child1) = children.get_mut(0) {
                            child1.resize(cols, rows1)?;
                        }
                        if let Some(child2) = children.get_mut(1) {
                            child2.resize(cols, rows2)?;
                        }
                    }
                    SplitDirection::Vertical => {
                        // Split cols between panes
                        let cols1 = (cols as f32 * *ratio) as usize;
                        let cols2 = cols.saturating_sub(cols1);
                        if let Some(child1) = children.get_mut(0) {
                            child1.resize(cols1, rows)?;
                        }
                        if let Some(child2) = children.get_mut(1) {
                            child2.resize(cols2, rows)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
