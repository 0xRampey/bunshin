use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal,
};
use std::io::{self, Write};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout, Command},
    select,
    signal::unix::{signal, SignalKind},
    time::{self, Duration},
};
use std::process::Stdio;

use crate::overlay::{self, OverlayState};

pub struct ShpoolProxy {
    session_name: String,
    worktree_path: PathBuf,
    branch_name: String,
    child: Option<Child>,
    overlay_state: OverlayState,
}

impl ShpoolProxy {
    pub fn new(session_name: String, worktree_path: PathBuf, branch_name: String) -> Self {
        let overlay_state = OverlayState::new(
            session_name.clone(),
            worktree_path.display().to_string(),
            branch_name.clone(),
        );

        Self {
            session_name,
            worktree_path,
            branch_name,
            child: None,
            overlay_state,
        }
    }

    /// Start the proxy with pass-through to Claude Code
    pub async fn start(&mut self, claude_binary: PathBuf) -> Result<()> {
        println!("🚀 Starting Bunshin session with overlay...");
        println!("📁 Session: {} | Branch: {}", self.session_name, self.branch_name);
        println!();
        println!("💡 Press Ctrl-~ to open the overlay menu");
        println!();

        // Launch Claude Code as a child process
        let mut child = Command::new(&claude_binary)
            .current_dir(&self.worktree_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env("BUNSHIN_SESSION", &self.session_name)
            .env("BUNSHIN_WORKTREE", self.worktree_path.display().to_string())
            .env("BUNSHIN_BRANCH", &self.branch_name)
            .spawn()?;

        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        let mut stdout = child.stdout.take().expect("Failed to open stdout");

        self.child = Some(child);

        // Enable raw mode
        terminal::enable_raw_mode()?;

        // Set up signal handlers
        let mut winch = signal(SignalKind::window_change())?;

        // Run the main proxy loop
        let result = self.proxy_loop(&mut stdin, &mut stdout, &mut winch).await;

        // Clean up
        terminal::disable_raw_mode()?;

        result
    }

    async fn proxy_loop(
        &mut self,
        stdin: &mut ChildStdin,
        stdout: &mut ChildStdout,
        winch: &mut tokio::signal::unix::Signal,
    ) -> Result<()> {
        let mut overlay_active = false;
        let mut buf = vec![0u8; 4096];

        loop {
            select! {
                // Forward output from Claude Code to terminal
                result = stdout.read(&mut buf), if !overlay_active => {
                    match result {
                        Ok(0) => {
                            // EOF - Claude Code exited
                            println!("\n✅ Claude Code session ended");
                            break;
                        }
                        Ok(n) => {
                            io::stdout().write_all(&buf[..n])?;
                            io::stdout().flush()?;
                        }
                        Err(e) => {
                            eprintln!("Error reading from Claude Code: {}", e);
                            break;
                        }
                    }
                }

                // Forward input from user to Claude Code
                result = tokio::task::spawn_blocking(|| event::read()), if !overlay_active => {
                    match result? {
                        Ok(Event::Key(KeyEvent { code: KeyCode::Char('~'), modifiers, .. }))
                            if modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Toggle overlay
                            overlay_active = true;
                            if let Ok(should_quit) = overlay::enter_overlay_ui(&self.overlay_state) {
                                if should_quit {
                                    println!("\n👋 Exiting session...");
                                    break;
                                }
                            }
                            overlay_active = false;
                        }
                        Ok(evt) => {
                            // Forward event as VT bytes
                            if let Some(bytes) = encode_event(evt) {
                                stdin.write_all(&bytes).await?;
                                stdin.flush().await?;
                            }
                        }
                        Err(e) => {
                            eprintln!("Error reading input: {}", e);
                            break;
                        }
                    }
                }

                // Handle window resize
                _ = winch.recv() => {
                    // Window was resized
                    // TODO: Send resize event to Claude Code if needed
                }
            }
        }

        Ok(())
    }
}

impl Drop for ShpoolProxy {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}

/// Encode crossterm event to VT100 bytes
fn encode_event(evt: Event) -> Option<Vec<u8>> {
    match evt {
        Event::Key(KeyEvent { code, modifiers, .. }) => {
            let mut bytes = Vec::new();

            // Handle common key codes
            match code {
                KeyCode::Char(c) => {
                    if modifiers.contains(KeyModifiers::CONTROL) {
                        // Ctrl+letter = ASCII control code
                        if c.is_ascii_alphabetic() {
                            let ctrl_code = (c.to_ascii_uppercase() as u8) - b'A' + 1;
                            bytes.push(ctrl_code);
                        } else {
                            bytes.push(c as u8);
                        }
                    } else if modifiers.contains(KeyModifiers::ALT) {
                        bytes.push(0x1b); // ESC
                        bytes.push(c as u8);
                    } else {
                        bytes.extend_from_slice(c.to_string().as_bytes());
                    }
                }
                KeyCode::Enter => bytes.push(b'\r'),
                KeyCode::Backspace => bytes.push(0x7f),
                KeyCode::Tab => bytes.push(b'\t'),
                KeyCode::Esc => bytes.push(0x1b),
                KeyCode::Up => bytes.extend_from_slice(b"\x1b[A"),
                KeyCode::Down => bytes.extend_from_slice(b"\x1b[B"),
                KeyCode::Right => bytes.extend_from_slice(b"\x1b[C"),
                KeyCode::Left => bytes.extend_from_slice(b"\x1b[D"),
                KeyCode::Home => bytes.extend_from_slice(b"\x1b[H"),
                KeyCode::End => bytes.extend_from_slice(b"\x1b[F"),
                KeyCode::PageUp => bytes.extend_from_slice(b"\x1b[5~"),
                KeyCode::PageDown => bytes.extend_from_slice(b"\x1b[6~"),
                KeyCode::Delete => bytes.extend_from_slice(b"\x1b[3~"),
                KeyCode::Insert => bytes.extend_from_slice(b"\x1b[2~"),
                KeyCode::F(n) => {
                    // F1-F12
                    let seq: &[u8] = match n {
                        1 => b"\x1bOP",
                        2 => b"\x1bOQ",
                        3 => b"\x1bOR",
                        4 => b"\x1bOS",
                        5 => b"\x1b[15~",
                        6 => b"\x1b[17~",
                        7 => b"\x1b[18~",
                        8 => b"\x1b[19~",
                        9 => b"\x1b[20~",
                        10 => b"\x1b[21~",
                        11 => b"\x1b[23~",
                        12 => b"\x1b[24~",
                        _ => b"",
                    };
                    bytes.extend_from_slice(seq);
                }
                _ => return None,
            }

            Some(bytes)
        }
        Event::Paste(text) => Some(text.into_bytes()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_char() {
        let evt = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        let bytes = encode_event(evt).unwrap();
        assert_eq!(bytes, b"a");
    }

    #[test]
    fn test_encode_ctrl_char() {
        let evt = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        let bytes = encode_event(evt).unwrap();
        assert_eq!(bytes, vec![3]); // Ctrl-C = 0x03
    }

    #[test]
    fn test_encode_arrow_up() {
        let evt = Event::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        let bytes = encode_event(evt).unwrap();
        assert_eq!(bytes, b"\x1b[A");
    }
}
