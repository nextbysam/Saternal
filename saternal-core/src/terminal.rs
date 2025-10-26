use alacritty_terminal::{
    event::{EventListener, OnResize},
    grid::Dimensions,
    term::{test::TermSize, Config as TermConfig, Term},
    tty::{self, EventedReadWrite},
    vte::ansi::Processor,
};
use anyhow::Result;
use log::{debug, info};
use parking_lot::Mutex;
use std::{collections::HashMap, sync::Arc};

/// Wrapper around Alacritty's terminal emulator
pub struct Terminal {
    term: Arc<Mutex<Term<TermEventListener>>>,
    pty: tty::Pty,
    processor: Processor,
}

impl Terminal {
    /// Create a new terminal with the specified dimensions
    pub fn new(cols: usize, rows: usize, shell: Option<String>) -> Result<Self> {
        info!("Creating new terminal: {}x{}", cols, rows);

        // Create PTY with WindowSize
        let mut env = HashMap::new();
        // Set TERM environment variable for proper shell initialization
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        // Inherit PATH and other important env vars
        if let Ok(path) = std::env::var("PATH") {
            env.insert("PATH".to_string(), path);
        }
        if let Ok(home) = std::env::var("HOME") {
            env.insert("HOME".to_string(), home);
        }
        if let Ok(user) = std::env::var("USER") {
            env.insert("USER".to_string(), user);
        }
        
        let pty_config = tty::Options {
            shell: shell.map(|s| tty::Shell::new(s, vec![])),
            working_directory: std::env::current_dir().ok(),
            drain_on_exit: true,
            env,
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
            pty,
            processor,
        })
    }

    /// Get reference to the terminal
    pub fn term(&self) -> Arc<Mutex<Term<TermEventListener>>> {
        self.term.clone()
    }

    /// Get the PTY for I/O operations
    pub fn pty(&self) -> &tty::Pty {
        &self.pty
    } //what does PTY mean here, and what are expeting in return in this function?
    //PTY means Pseudo Terminal, and we are expecting a reference to the PTY
    //this is a reference to the PTY, so we can use it to read and write to the PTY
    //the PTY is a trait that implements the EventedPty trait, which is a trait that
    //implements the Reader and Writer traits
    //the Reader trait is a trait that implements the read method
    //the Writer trait is a trait that implements the write method
    //the EventedPty trait is a trait that implements the on_resize method

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
    /// Returns the number of bytes processed
    pub fn process_output(&mut self) -> Result<usize> {
        use std::io::Read;

        let mut buf = [0u8; 4096];
        let mut total_bytes = 0;
        loop {
            match self.pty.reader().read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    total_bytes += n;
                    debug!("Read {} bytes from PTY: {:?}", n, String::from_utf8_lossy(&buf[..n]));
                    let mut term = self.term.lock();
                    self.processor.advance(&mut *term, &buf[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    debug!("PTY read error: {}", e);
                    return Err(e.into());
                }
            }
        }

        Ok(total_bytes)
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
