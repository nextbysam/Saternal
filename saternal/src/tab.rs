use anyhow::Result;
use log::info;
use saternal_core::{PaneNode, SplitDirection};

/// Represents a single tab containing a pane tree
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub pane_tree: PaneNode,
    next_pane_id: usize,
    /// Pending natural language commands awaiting user confirmation
    pub pending_nl_commands: Option<Vec<String>>,
    /// Whether the tab is in NL confirmation mode
    pub nl_confirmation_mode: bool,
}

impl Tab {
    pub fn new(id: usize, shell: Option<String>) -> Result<Self> {
        Self::new_with_size(id, 80, 24, shell)
    }
    
    pub fn new_with_size(id: usize, cols: usize, rows: usize, shell: Option<String>) -> Result<Self> {
        // Start with a single pane
        let pane_tree = PaneNode::new_leaf(0, cols, rows, shell)?;

        Ok(Self {
            id,
            title: format!("Tab {}", id + 1),
            pane_tree,
            next_pane_id: 1,
            pending_nl_commands: None,
            nl_confirmation_mode: false,
        })
    }

    /// Split the focused pane
    pub fn split(&mut self, direction: SplitDirection, shell: Option<String>) -> Result<()> {
        let pane_id = self.next_pane_id;
        self.next_pane_id += 1;

        if !self.pane_tree.split_focused(direction, pane_id, shell)? {
            log::warn!("No focused pane found to split");
        }

        Ok(())
    }

    /// Close the focused pane
    pub fn close_focused_pane(&mut self) -> Result<()> {
        // Don't close if it's the last pane
        if self.pane_tree.pane_ids().len() <= 1 {
            log::info!("Cannot close last pane");
            return Ok(());
        }

        if self.pane_tree.close_focused()? {
            // Focus the next available pane
            if let Some(first_id) = self.pane_tree.pane_ids().first() {
                self.pane_tree.set_focus(*first_id);
            }
        }

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
    /// Returns the total number of bytes processed across all panes
    pub fn process_output(&mut self) -> Result<usize> {
        // Process PTY output from ALL panes, not just focused
        // This ensures inactive panes continue to show live updates
        let panes = self.pane_tree.all_panes_mut();
        let mut total_bytes = 0;
        for (_pane_id, pane) in panes {
            // Ignore errors for individual panes (e.g., if PTY is closed)
            match pane.terminal.process_output() {
                Ok(bytes) => total_bytes += bytes,
                Err(e) => {
                    log::debug!("Output processing error: {}", e);
                }
            }
        }
        Ok(total_bytes)
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
        Self::new_with_size(shell, 80, 24)
    }
    
    pub fn new_with_size(shell: String, cols: usize, rows: usize) -> Result<Self> {
        // Start with one tab at the specified size
        let mut tab = Tab::new_with_size(0, cols, rows, Some(shell.clone()))?;

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

        Ok(id)
    }

    /// Close a tab
    pub fn close_tab(&mut self, id: usize) {
        if self.tabs.len() > 1 {
            self.tabs.retain(|tab| tab.id != id);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }

    /// Switch to a specific tab
    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
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
