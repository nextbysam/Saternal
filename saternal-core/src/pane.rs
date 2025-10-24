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
        // Take ownership of self
        let old_node = std::mem::replace(
            self,
            PaneNode::Leaf {
                pane: Pane::new(0, 1, 1, None)?,
            },
        );

        // Create new pane
        let new_pane = Pane::new(new_id, cols, rows, shell)?;
        let new_node = PaneNode::Leaf { pane: new_pane };

        // Create split with old and new nodes
        *self = PaneNode::Split {
            direction,
            children: vec![old_node, new_node],
            ratio: 0.5,
        };

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
