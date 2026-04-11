#[cfg(not(any(unix, windows)))]
mod dummy {
    use crate::editor::terminal_buffer::Cell;

    /// Stub terminal inner for unsupported platforms.
    pub struct DummyInner {
        pub running: bool,
    }

    impl DummyInner {
        pub fn write(&mut self, _data: &[u8]) -> Result<(), ()> {
            Ok(())
        }
        pub fn poll(&mut self) {}
        pub fn read(&mut self, _max: usize) -> Option<Vec<u8>> {
            None
        }
        pub fn cleanup(&mut self) {}
    }

    /// Stub terminal buffer (matches TerminalBufferInner API surface).
    pub struct DummyBuf {
        empty_screen: Vec<Vec<Cell>>,
    }

    impl DummyBuf {
        pub fn new() -> Self {
            Self {
                empty_screen: Vec::new(),
            }
        }
        pub fn process_output(&mut self, _bytes: &[u8]) {}
        pub fn resize(&mut self, _cols: usize, _rows: usize) {}
        pub fn screen(&self) -> &Vec<Vec<Cell>> {
            &self.empty_screen
        }
        pub fn cursor_row(&self) -> usize {
            0
        }
        pub fn cursor_col(&self) -> usize {
            0
        }
    }

    /// Stub terminal instance for non-Unix platforms.
    pub struct TerminalInstance {
        pub inner: DummyInner,
        pub tbuf: DummyBuf,
        pub title: String,
    }

    /// Stub terminal panel for unsupported platforms.
    pub struct TerminalPanel {
        pub terminals: Vec<TerminalInstance>,
        pub active: usize,
        pub visible: bool,
        pub focused: bool,
    }

    impl TerminalPanel {
        pub fn new() -> Self {
            Self {
                terminals: vec![],
                active: 0,
                visible: false,
                focused: false,
            }
        }

        pub fn spawn(&mut self, _root: &str) -> bool {
            false
        }

        pub fn close_active(&mut self) -> bool {
            false
        }

        pub fn active_terminal(&mut self) -> Option<&mut TerminalInstance> {
            None
        }

        pub fn next_tab(&mut self) {}

        pub fn prev_tab(&mut self) {}
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) use dummy::*;

#[cfg(unix)]
mod unix_impl {
    use std::ffi::CString;

    use crate::editor::terminal::{
        TerminalInner, TerminalSpawnOptions, ensure_terminal_env, spawn_terminal,
    };
    use crate::editor::terminal_buffer::{DEFAULT_SCROLLBACK, TerminalBufferInner};

    pub(crate) const MAX_TERMINALS: usize = 10;

    /// Single terminal instance within the panel.
    pub(crate) struct TerminalInstance {
        pub(crate) inner: TerminalInner,
        pub(crate) tbuf: TerminalBufferInner,
        pub(crate) title: String,
    }

    /// Standard 16-color ANSI palette.
    pub(crate) fn default_16_color_palette() -> [[u8; 4]; 16] {
        [
            [0, 0, 0, 255],       // black
            [170, 0, 0, 255],     // red
            [0, 170, 0, 255],     // green
            [170, 85, 0, 255],    // yellow/brown
            [0, 0, 170, 255],     // blue
            [170, 0, 170, 255],   // magenta
            [0, 170, 170, 255],   // cyan
            [170, 170, 170, 255], // white
            [85, 85, 85, 255],    // bright black
            [255, 85, 85, 255],   // bright red
            [85, 255, 85, 255],   // bright green
            [255, 255, 85, 255],  // bright yellow
            [85, 85, 255, 255],   // bright blue
            [255, 85, 255, 255],  // bright magenta
            [85, 255, 255, 255],  // bright cyan
            [255, 255, 255, 255], // bright white
        ]
    }

    /// Multi-terminal panel managing several terminal instances.
    pub(crate) struct TerminalPanel {
        pub(crate) terminals: Vec<TerminalInstance>,
        pub(crate) active: usize,
        pub(crate) visible: bool,
        pub(crate) focused: bool,
    }

    impl TerminalPanel {
        pub(crate) fn new() -> Self {
            Self {
                terminals: Vec::new(),
                active: 0,
                visible: false,
                focused: false,
            }
        }

        /// Spawn a new terminal instance. Returns false if at the limit.
        pub(crate) fn spawn(&mut self, project_root: &str) -> bool {
            if self.terminals.len() >= MAX_TERMINALS {
                return false;
            }
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let cmd = vec![CString::new(shell).unwrap_or_default()];
            let mut env = Vec::new();
            let _ = ensure_terminal_env(&mut env);
            let opts = TerminalSpawnOptions {
                cwd: Some(CString::new(project_root).unwrap_or_default()),
                env,
                cols: 80,
                rows: 24,
            };
            match spawn_terminal(&cmd, &opts) {
                Ok(inner) => {
                    let idx = self.terminals.len();
                    let title = format!("Terminal {}", idx + 1);
                    let palette = default_16_color_palette();
                    let default_fg = [200, 200, 200, 255];
                    let tbuf =
                        TerminalBufferInner::new(80, 24, DEFAULT_SCROLLBACK, palette, default_fg);
                    let inst = TerminalInstance { inner, tbuf, title };
                    self.terminals.push(inst);
                    self.active = idx;
                    self.visible = true;
                    self.focused = true;
                    true
                }
                Err(e) => {
                    eprintln!("[terminal] Spawn failed: {e}");
                    false
                }
            }
        }

        /// Close the active terminal. Returns true if panel should stay visible.
        pub(crate) fn close_active(&mut self) -> bool {
            if self.terminals.is_empty() {
                return false;
            }
            self.terminals[self.active].inner.cleanup();
            self.terminals.remove(self.active);
            if self.terminals.is_empty() {
                self.active = 0;
                self.visible = false;
                self.focused = false;
                return false;
            }
            if self.active >= self.terminals.len() {
                self.active = self.terminals.len() - 1;
            }
            true
        }

        /// Get the active terminal instance, if any.
        pub(crate) fn active_terminal(&mut self) -> Option<&mut TerminalInstance> {
            self.terminals.get_mut(self.active)
        }

        /// Switch to next terminal tab.
        pub(crate) fn next_tab(&mut self) {
            if !self.terminals.is_empty() {
                self.active = (self.active + 1) % self.terminals.len();
            }
        }

        /// Switch to previous terminal tab.
        pub(crate) fn prev_tab(&mut self) {
            if !self.terminals.is_empty() {
                self.active = if self.active == 0 {
                    self.terminals.len() - 1
                } else {
                    self.active - 1
                };
            }
        }
    }
}

#[cfg(unix)]
pub(crate) use unix_impl::*;

#[cfg(windows)]
mod windows_impl {
    use crate::editor::terminal_buffer::{DEFAULT_SCROLLBACK, TerminalBufferInner};
    use crate::editor::terminal_windows::{
        TerminalInner, TerminalSpawnOptions, ensure_terminal_env, spawn_terminal,
    };

    pub(crate) const MAX_TERMINALS: usize = 10;

    /// Single terminal instance within the panel.
    pub(crate) struct TerminalInstance {
        pub(crate) inner: TerminalInner,
        pub(crate) tbuf: TerminalBufferInner,
        pub(crate) title: String,
    }

    /// Standard 16-color ANSI palette.
    pub(crate) fn default_16_color_palette() -> [[u8; 4]; 16] {
        [
            [0, 0, 0, 255],       // black
            [170, 0, 0, 255],     // red
            [0, 170, 0, 255],     // green
            [170, 85, 0, 255],    // yellow/brown
            [0, 0, 170, 255],     // blue
            [170, 0, 170, 255],   // magenta
            [0, 170, 170, 255],   // cyan
            [170, 170, 170, 255], // white
            [85, 85, 85, 255],    // bright black
            [255, 85, 85, 255],   // bright red
            [85, 255, 85, 255],   // bright green
            [255, 255, 85, 255],  // bright yellow
            [85, 85, 255, 255],   // bright blue
            [255, 85, 255, 255],  // bright magenta
            [85, 255, 255, 255],  // bright cyan
            [255, 255, 255, 255], // bright white
        ]
    }

    /// Multi-terminal panel managing several terminal instances.
    pub(crate) struct TerminalPanel {
        pub(crate) terminals: Vec<TerminalInstance>,
        pub(crate) active: usize,
        pub(crate) visible: bool,
        pub(crate) focused: bool,
    }

    impl TerminalPanel {
        pub(crate) fn new() -> Self {
            Self {
                terminals: Vec::new(),
                active: 0,
                visible: false,
                focused: false,
            }
        }

        /// Spawn a new terminal instance. Returns false if at the limit.
        pub(crate) fn spawn(&mut self, project_root: &str) -> bool {
            if self.terminals.len() >= MAX_TERMINALS {
                return false;
            }
            let mut env = Vec::new();
            let _ = ensure_terminal_env(&mut env);
            let opts = TerminalSpawnOptions {
                cwd: Some(project_root.to_string()),
                env,
                cols: 80,
                rows: 24,
            };
            match spawn_terminal(&opts) {
                Ok(inner) => {
                    let idx = self.terminals.len();
                    let title = format!("Terminal {}", idx + 1);
                    let palette = default_16_color_palette();
                    let default_fg = [200, 200, 200, 255];
                    let tbuf =
                        TerminalBufferInner::new(80, 24, DEFAULT_SCROLLBACK, palette, default_fg);
                    let inst = TerminalInstance { inner, tbuf, title };
                    self.terminals.push(inst);
                    self.active = idx;
                    self.visible = true;
                    self.focused = true;
                    true
                }
                Err(e) => {
                    eprintln!("[terminal] Spawn failed: {e}");
                    false
                }
            }
        }

        /// Close the active terminal. Returns true if panel should stay visible.
        pub(crate) fn close_active(&mut self) -> bool {
            if self.terminals.is_empty() {
                return false;
            }
            self.terminals[self.active].inner.cleanup();
            self.terminals.remove(self.active);
            if self.terminals.is_empty() {
                self.active = 0;
                self.visible = false;
                self.focused = false;
                return false;
            }
            if self.active >= self.terminals.len() {
                self.active = self.terminals.len() - 1;
            }
            true
        }

        /// Get the active terminal instance, if any.
        pub(crate) fn active_terminal(&mut self) -> Option<&mut TerminalInstance> {
            self.terminals.get_mut(self.active)
        }

        /// Switch to next terminal tab.
        pub(crate) fn next_tab(&mut self) {
            if !self.terminals.is_empty() {
                self.active = (self.active + 1) % self.terminals.len();
            }
        }

        /// Switch to previous terminal tab.
        pub(crate) fn prev_tab(&mut self) {
            if !self.terminals.is_empty() {
                self.active = if self.active == 0 {
                    self.terminals.len() - 1
                } else {
                    self.active - 1
                };
            }
        }
    }
}

#[cfg(windows)]
pub(crate) use windows_impl::*;
