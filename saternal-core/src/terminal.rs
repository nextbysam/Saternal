use alacritty_terminal::{
    event::{EventListener, OnResize},
    grid::Dimensions,
    term::{test::TermSize, Config as TermConfig, Term},
    tty,
    vte::ansi::Processor,
};
use anyhow::Result;
use log::{debug, info};
use parking_lot::Mutex;
use std::{collections::HashMap, fs::File, sync::Arc};

/// Wrapper around Alacritty's terminal emulator
pub struct Terminal {
    term: Arc<Mutex<Term<TermEventListener>>>,
    pty: Box<dyn tty::EventedPty<Reader = File, Writer = File>>,
    processor: Processor,
}

impl Terminal {
    /// Create a new terminal with the specified dimensions
    pub fn new(cols: usize, rows: usize, shell: Option<String>) -> Result<Self> {
        info!("Creating new terminal: {}x{}", cols, rows);

        // Create PTY with WindowSize
        let pty_config = tty::Options {
            shell: shell.map(|s| tty::Shell::new(s, vec![])),
            working_directory: None,
            drain_on_exit: true,
            env: HashMap::new(),
        };

        let window_size = alacritty_terminal::event::WindowSize {
            num_cols: cols as u16,
            num_lines: rows as u16,
            cell_width: 8,
            cell_height: 16,
        };

        let pty = tty::new(&pty_config, window_size, 0)?;

        // Create terminal with TermSize
        let event_listener = TermEventListener::new();
        let size = TermSize::new(cols, rows);
        let term = Term::new(TermConfig::default(), &size, event_listener);

        let term = Arc::new(Mutex::new(term));

        // Create VTE processor
        let processor = Processor::new();

        Ok(Self {
            term,
            pty: Box::new(pty),
            processor,
        })
    }

    /// Get reference to the terminal
    pub fn term(&self) -> Arc<Mutex<Term<TermEventListener>>> {
        self.term.clone()
    }

    /// Get the PTY for I/O operations
    pub fn pty(&self) -> &dyn tty::EventedPty<Reader = File, Writer = File> {
        self.pty.as_ref()
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
        debug!("Resizing terminal to {}x{}", cols, rows);

        let size = TermSize::new(cols, rows);
        let mut term = self.term.lock();
        term.resize(size);

        let window_size = alacritty_terminal::event::WindowSize {
            num_cols: cols as u16,
            num_lines: rows as u16,
            cell_width: 8,
            cell_height: 16,
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
                    self.processor.advance(&mut *term, &buf[..n]);
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
    pub fn new() -> Self {
        Self {}
    }
}

impl EventListener for TermEventListener {
    fn send_event(&self, event: alacritty_terminal::event::Event) {
        debug!("Terminal event: {:?}", event);
        // Handle terminal events like title changes, etc.
    }
}
