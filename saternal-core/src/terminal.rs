use alacritty_terminal::{
    event::EventListener,
    event_loop::Notifier,
    grid::Dimensions,
    term::Term,
    tty,
};
use anyhow::Result;
use log::{debug, info};
use parking_lot::Mutex;
use std::sync::Arc;

/// Wrapper around Alacritty's terminal emulator
pub struct Terminal {
    term: Arc<Mutex<Term<TermEventListener>>>,
    pty: Box<dyn tty::EventedPty>,
    notifier: Notifier,
}

impl Terminal {
    /// Create a new terminal with the specified dimensions
    pub fn new(cols: usize, rows: usize, shell: Option<String>) -> Result<Self> {
        info!("Creating new terminal: {}x{}", cols, rows);

        // Create PTY
        let pty_config = tty::Options {
            shell: shell.map(|s| tty::Shell::new(s, vec![])),
            working_directory: None,
            hold: false,
        };

        let pty = tty::new(&pty_config, (cols as u16, rows as u16), 0)?;

        // Create terminal
        let event_listener = TermEventListener::new();
        let term = Term::new(
            &alacritty_terminal::term::Config::default(),
            &(cols as usize, rows as usize),
            event_listener,
        );

        let term = Arc::new(Mutex::new(term));

        // Create event loop notifier
        let notifier = Notifier::new();

        Ok(Self {
            term,
            pty,
            notifier,
        })
    }

    /// Get reference to the terminal
    pub fn term(&self) -> Arc<Mutex<Term<TermEventListener>>> {
        self.term.clone()
    }

    /// Get the PTY for I/O operations
    pub fn pty(&self) -> &dyn tty::EventedPty {
        self.pty.as_ref()
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
        debug!("Resizing terminal to {}x{}", cols, rows);

        let mut term = self.term.lock();
        term.resize((cols, rows));

        let window_size = alacritty_terminal::event::WindowSize {
            num_cols: cols as u16,
            num_lines: rows as u16,
            cell_width: 8,  // Will be updated by renderer
            cell_height: 16, // Will be updated by renderer
        };

        self.pty.on_resize(window_size);

        Ok(())
    }

    /// Write input to the terminal
    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        use std::io::Write;
        self.pty.writer().write_all(data)?;
        Ok(())
    }

    /// Read output from the terminal and process it
    pub fn process_output(&mut self) -> Result<()> {
        use std::io::Read;

        let mut buf = [0u8; 4096];
        loop {
            match self.pty.reader().read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let mut term = self.term.lock();
                    for byte in &buf[..n] {
                        term.advance(*byte);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    /// Get grid dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        let term = self.term.lock();
        (term.columns(), term.screen_lines())
    }
}

/// Event listener for terminal events
pub struct TermEventListener {
    // We can add fields here to track terminal events
    // For now, we just implement the required trait
}

impl TermEventListener {
    fn new() -> Self {
        Self {}
    }
}

impl EventListener for TermEventListener {
    fn send_event(&self, event: alacritty_terminal::event::Event) {
        debug!("Terminal event: {:?}", event);
        // Handle terminal events like title changes, etc.
    }
}
