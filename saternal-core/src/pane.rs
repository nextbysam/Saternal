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

    /// Get the focused pane, if any
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

    /// Resize all panes in the tree based on available space
    pub fn resize(&mut self, width: usize, height: usize) -> Result<()> {
        match self {
            PaneNode::Leaf { pane } => {
                // Calculate cols/rows based on cell size
                // For now, assume 8x16 cell size (will be updated by renderer)
                let cols = width / 8;
                let rows = height / 16;
                pane.resize(cols.max(1), rows.max(1))?;
            }
            PaneNode::Split {
                direction,
                children,
                ratio,
            } => {
                match direction {
                    SplitDirection::Horizontal => {
                        // Split height
                        let height1 = (height as f32 * *ratio) as usize;
                        let height2 = height.saturating_sub(height1);
                        if let Some(child1) = children.get_mut(0) {
                            child1.resize(width, height1)?;
                        }
                        if let Some(child2) = children.get_mut(1) {
                            child2.resize(width, height2)?;
                        }
                    }
                    SplitDirection::Vertical => {
                        // Split width
                        let width1 = (width as f32 * *ratio) as usize;
                        let width2 = width.saturating_sub(width1);
                        if let Some(child1) = children.get_mut(0) {
                            child1.resize(width1, height)?;
                        }
                        if let Some(child2) = children.get_mut(1) {
                            child2.resize(width2, height)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
