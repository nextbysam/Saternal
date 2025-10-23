use anyhow::Result;
use log::info;
use saternal_core::{PaneNode, SplitDirection};

/// Represents a single tab containing a pane tree
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub pane_tree: PaneNode,
    next_pane_id: usize,
}

impl Tab {
    pub fn new(id: usize, shell: Option<String>) -> Result<Self> {
        info!("Creating new tab: {}", id);

        // Start with a single pane
        let pane_tree = PaneNode::new_leaf(0, 80, 24, shell)?;

        Ok(Self {
            id,
            title: format!("Tab {}", id + 1),
            pane_tree,
            next_pane_id: 1,
        })
    }

    /// Split the focused pane
    pub fn split(&mut self, direction: SplitDirection, shell: Option<String>) -> Result<()> {
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        // For now, split the root
        // TODO: Split only the focused pane
        self.pane_tree.split(direction, pane_id, 80, 24, shell)?;

        Ok(())
    }

    /// Write input to the focused pane
    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        if let Some(pane) = self.pane_tree.focused_pane_mut() {
            pane.terminal.write_input(data)?;
        }
        Ok(())
    }

    /// Process output from all panes
    pub fn process_output(&mut self) -> Result<()> {
        // TODO: Process output from all panes, not just focused
        if let Some(pane) = self.pane_tree.focused_pane_mut() {
            log::trace!("Tab {}: Processing pane output", self.id);
            pane.terminal.process_output()?;
        } else {
            log::warn!("Tab {}: No focused pane found", self.id);
        }
        Ok(())
    }

    /// Resize the tab to fit new dimensions
    pub fn resize(&mut self, width: usize, height: usize) -> Result<()> {
        self.pane_tree.resize(width, height)
    }
}

/// Manages multiple tabs
pub struct TabManager {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: usize,
    shell: String,
}

impl TabManager {
    pub fn new(shell: String) -> Result<Self> {
        info!("Creating tab manager");

        // Start with one tab
        let mut tab = Tab::new(0, Some(shell.clone()))?;

        // Set first pane as focused
        tab.pane_tree.set_focus(0);

        Ok(Self {
            tabs: vec![tab],
            active_tab: 0,
            next_tab_id: 1,
            shell,
        })
    }

    /// Create a new tab
    pub fn new_tab(&mut self) -> Result<usize> {
        let id = self.next_tab_id;
        self.next_tab_id += 1;

        let mut tab = Tab::new(id, Some(self.shell.clone()))?;
        tab.pane_tree.set_focus(0);

        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;

        info!("Created new tab: {}", id);
        Ok(id)
    }

    /// Close a tab
    pub fn close_tab(&mut self, id: usize) {
        if self.tabs.len() > 1 {
            self.tabs.retain(|tab| tab.id != id);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            info!("Closed tab: {}", id);
        } else {
            info!("Cannot close last tab");
        }
    }

    /// Switch to a specific tab
    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
            info!("Switched to tab: {}", index);
        }
    }

    /// Get the active tab
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab)
    }

    /// Get the active tab mutably
    pub fn active_tab_mut(&mut self) -> Option<&mut Tab> {
        self.tabs.get_mut(self.active_tab)
    }

    /// Get all tabs
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Get number of tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
}
