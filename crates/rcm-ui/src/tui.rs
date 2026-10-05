use std::io::{stdout, Write};
use std::time::Duration;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use rcm_core::DaemonState;
use rcm_ui_kit::view_model::SidebarDestination;
use crate::app::AppController;

pub struct TuiApp {
    controller: AppController,
    should_quit: bool,
    selected_index: usize,
    status_message: Option<(String, std::time::Instant)>,
}

impl TuiApp {
    pub fn new(controller: AppController) -> Self {
        Self {
            controller,
            should_quit: false,
            selected_index: 0,
            status_message: None,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), std::time::Instant::now()));
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, Hide)?;

        let res = self.main_loop().await;

        execute!(out, Show, LeaveAlternateScreen)?;
        disable_raw_mode()?;

        res
    }

    async fn main_loop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Refresh data on startup
        self.refresh_data().await;

        loop {
            self.render().await?;

            if event::poll(Duration::from_millis(150))? {
                if let Event::Key(key) = event::read()? {
                    // Global shortcut: Ctrl+C or 'q' when not in input mode
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        break;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('k') {
                        self.controller.toggle_command_palette().await;
                        continue;
                    }

                    let is_palette_open = self.controller.state().read().await.command_palette_open;

                    if is_palette_open {
                        match key.code {
                            KeyCode::Esc => {
                                self.controller.toggle_command_palette().await;
                            }
                            KeyCode::Enter => {
                                let items = self.controller.get_command_palette_items().await;
                                if let Some(item) = items.get(self.selected_index) {
                                    if let Some(dest) = item.destination {
                                        self.controller.switch_destination(dest).await;
                                        self.selected_index = 0;
                                    }
                                }
                                self.controller.toggle_command_palette().await;
                            }
                            KeyCode::Up => {
                                if self.selected_index > 0 {
                                    self.selected_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                let items = self.controller.get_command_palette_items().await;
                                if self.selected_index + 1 < items.len() {
                                    self.selected_index += 1;
                                }
                            }
                            KeyCode::Backspace => {
                                let mut query = self.controller.state().read().await.command_palette_query.clone();
                                query.pop();
                                self.controller.set_command_palette_query(query).await;
                                self.selected_index = 0;
                            }
                            KeyCode::Char(c) => {
                                let mut query = self.controller.state().read().await.command_palette_query.clone();
                                query.push(c);
                                self.controller.set_command_palette_query(query).await;
                                self.selected_index = 0;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Main screen navigation
                    match key.code {
                        KeyCode::Char('q') => {
                            self.should_quit = true;
                            break;
                        }
                        KeyCode::Char('1') => {
                            self.controller.switch_destination(SidebarDestination::Home).await;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('2') => {
                            self.controller.switch_destination(SidebarDestination::Remotes).await;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('3') => {
                            self.controller.switch_destination(SidebarDestination::Mounts).await;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('4') => {
                            self.controller.switch_destination(SidebarDestination::Serves).await;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('5') => {
                            self.controller.switch_destination(SidebarDestination::Files).await;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('6') => {
                            self.controller.switch_destination(SidebarDestination::Settings).await;
                            self.selected_index = 0;
                        }
                        KeyCode::Char('r') => {
                            self.refresh_data().await;
                            self.set_status("Refreshed all status and profiles.");
                        }
                        KeyCode::Up => {
                            if self.selected_index > 0 {
                                self.selected_index -= 1;
                            }
                        }
                        KeyCode::Down => {
                            self.selected_index += 1;
                        }
                        KeyCode::Enter => {
                            self.handle_enter_action().await;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }

    async fn refresh_data(&mut self) {
        let state_arc = self.controller.state();
        let s = state_arc.read().await;

        if let Some(ref ipc) = s.ipc_client {
            // Update daemon status
            if let Ok(st_val) = ipc.call("daemon.status", serde_json::json!({})).await {
                if let Ok(st) = serde_json::from_value::<DaemonState>(st_val) {
                    drop(s);
                    state_arc.write().await.daemon_state = st;
                }
            }
        }
    }

    async fn handle_enter_action(&mut self) {
        let dest = self.controller.state().read().await.current_destination;
        match dest {
            SidebarDestination::Home => {
                self.refresh_data().await;
                self.set_status("Dashboard refreshed.");
            }
            SidebarDestination::Mounts => {
                self.set_status("Triggered mount reconciliation.");
                if let Some(ref ipc) = self.controller.state().read().await.ipc_client {
                    let _ = ipc.call("reconcile.now", serde_json::json!({})).await;
                }
            }
            SidebarDestination::Settings => {
                self.set_status("Triggered daemon restart.");
                if let Some(ref ipc) = self.controller.state().read().await.ipc_client {
                    let _ = ipc.call("daemon.restart", serde_json::json!({})).await;
                }
            }
            _ => {}
        }
    }

    async fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut out = stdout();
        execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;

        let (width, height) = crossterm::terminal::size().unwrap_or((80, 24));
        let state_arc = self.controller.state();
        let state_guard = state_arc.read().await;
        let state = &*state_guard;

        // 1. Header Bar
        self.render_header(&mut out, state, width)?;

        // 2. Navigation Bar
        self.render_nav(&mut out, state.current_destination, width)?;

        // 3. Body View
        let body_height = height.saturating_sub(6) as usize;
        match state.current_destination {
            SidebarDestination::Home => self.render_dashboard(&mut out, state, body_height)?,
            SidebarDestination::Remotes => self.render_remotes(&mut out, state, body_height)?,
            SidebarDestination::Mounts => self.render_mounts(&mut out, state, body_height)?,
            SidebarDestination::Serves => self.render_serves(&mut out, state, body_height)?,
            SidebarDestination::Files => self.render_files(&mut out, state, body_height)?,
            SidebarDestination::Settings => self.render_settings(&mut out, state, body_height)?,
        }

        // 4. Command Palette Overlay (if active)
        if state.command_palette_open {
            let items = self.controller.get_command_palette_items().await;
            self.render_command_palette(&mut out, state, &items, width, height)?;
        }

        // 5. Footer & Status Bar
        self.render_footer(&mut out, width, height)?;

        out.flush()?;
        Ok(())
    }

    fn render_header(&self, out: &mut std::io::Stdout, state: &crate::app::AppState, width: u16) -> Result<(), std::io::Error> {
        execute!(
            out,
            SetBackgroundColor(Color::Rgb { r: 30, g: 30, b: 46 }),
            SetForegroundColor(Color::White)
        )?;

        let daemon_str = match &state.daemon_state {
            DaemonState::Ready { version, pid, addr, .. } => {
                format!(" ● rclone {} [READY] (PID {}) on {}", version, pid, addr)
            }
            DaemonState::Stopped => " ○ rclone [STOPPED]".to_string(),
            DaemonState::Starting => " ◐ rclone [STARTING...]".to_string(),
            DaemonState::Stopping => " ◑ rclone [STOPPING...]".to_string(),
            DaemonState::Degraded { reason } => format!(" ⚠ rclone [DEGRADED: {}]", reason),
            DaemonState::Crashed { exit_code, .. } => format!(" ✖ rclone [CRASHED: code {:?}]", exit_code),
            DaemonState::Failed { reason } => format!(" ✖ rclone [FAILED: {}]", reason),
        };

        let right_part = " [Ctrl+K] Palette | [q] Close Window ";
        let pad_len = (width as usize).saturating_sub(daemon_str.len() + right_part.len());
        let padding = " ".repeat(pad_len);

        execute!(
            out,
            Print(format!("{}{}{}\n", daemon_str, padding, right_part)),
            ResetColor
        )?;
        Ok(())
    }

    fn render_nav(&self, out: &mut std::io::Stdout, current: SidebarDestination, width: u16) -> Result<(), std::io::Error> {
        let destinations = [
            (SidebarDestination::Home, "[1] Dashboard"),
            (SidebarDestination::Remotes, "[2] Remotes"),
            (SidebarDestination::Mounts, "[3] Mounts"),
            (SidebarDestination::Serves, "[4] Serves"),
            (SidebarDestination::Files, "[5] Files"),
            (SidebarDestination::Settings, "[6] Settings"),
        ];

        let mut nav_line = String::new();
        for (dest, label) in destinations {
            if dest == current {
                nav_line.push_str(&format!(" > {} <  ", label));
            } else {
                nav_line.push_str(&format!("   {}    ", label));
            }
        }

        execute!(
            out,
            SetForegroundColor(Color::Rgb { r: 137, g: 180, b: 250 }),
            Print(format!("{}\n", nav_line)),
            SetForegroundColor(Color::DarkGrey),
            Print(format!("{}\n", "─".repeat(width as usize))),
            ResetColor
        )?;
        Ok(())
    }

    fn render_dashboard(&self, out: &mut std::io::Stdout, state: &crate::app::AppState, _height: usize) -> Result<(), std::io::Error> {
        execute!(out, SetForegroundColor(Color::White))?;
        execute!(out, Print("  DASHBOARD & OVERVIEW\n\n"))?;

        execute!(
            out,
            SetForegroundColor(Color::Cyan),
            Print(format!("  • Background Daemon State:  {:?}\n", state.daemon_state)),
            Print(format!("  • Active Mounts:            {}\n", state.mounts.len())),
            Print(format!("  • Active Serves:            {}\n", state.serves.len())),
            Print("  • Configured Remotes:       Available via [2] Remotes\n\n"),
            SetForegroundColor(Color::Yellow),
            Print("  QUICK ACTIONS:\n"),
            Print("    [3] View & Mount Cloud Storage as Windows Drive Letters (G:, M:, ...)\n"),
            Print("    [2] Add or Manage Remotes with interactive CLI Wizard\n"),
            Print("    [4] Start Local WebDAV / SFTP / S3 Endpoints\n"),
            Print("    [6] Settings, Backups, Redacted Diffs & Logs\n"),
            Print("    [r] Refresh Status\n"),
            ResetColor
        )?;
        Ok(())
    }

    fn render_remotes(&self, out: &mut std::io::Stdout, _state: &crate::app::AppState, _height: usize) -> Result<(), std::io::Error> {
        execute!(
            out,
            SetForegroundColor(Color::White),
            Print("  REMOTES MANAGEMENT (Schema-driven, CLI-parity)\n\n"),
            SetForegroundColor(Color::Cyan),
            Print("  Configured Remotes:\n"),
            Print("    No remotes configured yet or refreshing...\n\n"),
            SetForegroundColor(Color::Yellow),
            Print("  Actions:\n"),
            Print("    [a] Add Remote (Interactive rclone config wizard)\n"),
            Print("    [d] Delete Remote (with dependency check)\n"),
            Print("    [c] Duplicate Remote\n"),
            Print("    [t] Test Connection\n"),
            ResetColor
        )?;
        Ok(())
    }

    fn render_mounts(&self, out: &mut std::io::Stdout, _state: &crate::app::AppState, _height: usize) -> Result<(), std::io::Error> {
        execute!(
            out,
            SetForegroundColor(Color::White),
            Print("  MOUNTS & VFS PROFILES (WinFsp / FUSE native drives)\n\n"),
            SetForegroundColor(Color::Cyan),
            Print("  Configured Mounts:\n"),
            Print("    (Profiles saved in %APPDATA%\\RCM\\profiles.toml)\n\n"),
            SetForegroundColor(Color::Yellow),
            Print("  Presets available:\n"),
            Print("    • Balanced:        Writes cache, 30m dir cache (everyday use)\n"),
            Print("    • Streaming:       Full cache, 256M read-ahead (media streaming)\n"),
            Print("    • Offline-first:   Full cache, 50G max size (work through disconnects)\n"),
            Print("    • Max Compat:      Network drive mode (Explorer compatibility)\n\n"),
            Print("  Actions:\n"),
            Print("    [Enter] Reconcile and start autostart mounts\n"),
            Print("    [n]     New Mount Profile\n"),
            ResetColor
        )?;
        Ok(())
    }

    fn render_serves(&self, out: &mut std::io::Stdout, _state: &crate::app::AppState, _height: usize) -> Result<(), std::io::Error> {
        execute!(
            out,
            SetForegroundColor(Color::White),
            Print("  SERVES (WebDAV / SFTP / HTTP / S3 / DLNA)\n\n"),
            SetForegroundColor(Color::Cyan),
            Print("  Available Protocols:\n"),
            Print("    • WebDAV: Share files over local network to iPad/TV/clients\n"),
            Print("    • SFTP:   Secure shell file transfer endpoint\n"),
            Print("    • HTTP:   Simple browser-viewable download service\n"),
            Print("    • S3:     S3-compatible API gateway\n\n"),
            SetForegroundColor(Color::Yellow),
            Print("  Security Guarantee: Non-loopback binds enforce authentication (SV-3)\n"),
            ResetColor
        )?;
        Ok(())
    }

    fn render_files(&self, out: &mut std::io::Stdout, _state: &crate::app::AppState, _height: usize) -> Result<(), std::io::Error> {
        execute!(
            out,
            SetForegroundColor(Color::White),
            Print("  FILES (Remote Explorer & Transfer Center)\n\n"),
            SetForegroundColor(Color::Cyan),
            Print("  Browse cloud storage directories using rclone RC operations/list.\n"),
            Print("  Fast, virtualized and responsive.\n\n"),
            SetForegroundColor(Color::Yellow),
            Print("  Select a remote to begin exploring.\n"),
            ResetColor
        )?;
        Ok(())
    }

    fn render_settings(&self, out: &mut std::io::Stdout, _state: &crate::app::AppState, _height: usize) -> Result<(), std::io::Error> {
        execute!(
            out,
            SetForegroundColor(Color::White),
            Print("  SETTINGS, BACKUPS & DAEMON CONTROLS\n\n"),
            SetForegroundColor(Color::Cyan),
            Print("  • rclone Binary:    Managed in %LOCALAPPDATA%\\RCM\\rclone\n"),
            Print("  • Config Path:      %APPDATA%\\rclone\\rclone.conf\n"),
            Print("  • Snapshot Store:   Automatic pre-mutation & restore snapshots\n\n"),
            SetForegroundColor(Color::Yellow),
            Print("  Actions:\n"),
            Print("    [Enter] Restart Daemon\n"),
            Print("    [s]     Stop Daemon\n"),
            Print("    [b]     Browse & Restore Snapshots\n"),
            Print("    [d]     View Redacted Diff against current config\n"),
            ResetColor
        )?;
        Ok(())
    }

    fn render_command_palette(
        &self,
        out: &mut std::io::Stdout,
        state: &crate::app::AppState,
        items: &[rcm_ui_kit::view_model::CommandPaletteItem],
        width: u16,
        height: u16,
    ) -> Result<(), std::io::Error> {
        let palette_width = (width * 3 / 4).max(40).min(width);
        let palette_height = 10.min(height.saturating_sub(4));
        let start_x = (width.saturating_sub(palette_width)) / 2;
        let start_y = (height.saturating_sub(palette_height)) / 2;

        for y in 0..palette_height {
            execute!(out, MoveTo(start_x, start_y + y))?;
            execute!(
                out,
                SetBackgroundColor(Color::Rgb { r: 49, g: 50, b: 68 }),
                SetForegroundColor(Color::White)
            )?;

            if y == 0 {
                let title = format!(" Search Commands: {}_ ", state.command_palette_query);
                let pad = (palette_width as usize).saturating_sub(title.len());
                execute!(out, Print(format!("{}{}", title, " ".repeat(pad))))?;
            } else if y == 1 {
                execute!(out, Print("─".repeat(palette_width as usize)))?;
            } else {
                let item_idx = (y - 2) as usize;
                if let Some(item) = items.get(item_idx) {
                    let is_sel = item_idx == self.selected_index;
                    let prefix = if is_sel { " > " } else { "   " };
                    let row = format!("{}{}: {}", prefix, item.title, item.subtitle.as_deref().unwrap_or(""));
                    let pad = (palette_width as usize).saturating_sub(row.len());
                    if is_sel {
                        execute!(
                            out,
                            SetBackgroundColor(Color::Rgb { r: 137, g: 180, b: 250 }),
                            SetForegroundColor(Color::Black)
                        )?;
                    }
                    execute!(out, Print(format!("{}{}", row, " ".repeat(pad))))?;
                } else {
                    execute!(out, Print(" ".repeat(palette_width as usize)))?;
                }
            }
        }

        execute!(out, ResetColor)?;
        Ok(())
    }

    fn render_footer(&self, out: &mut std::io::Stdout, width: u16, height: u16) -> Result<(), std::io::Error> {
        execute!(out, MoveTo(0, height.saturating_sub(2)))?;
        execute!(out, SetForegroundColor(Color::DarkGrey))?;
        execute!(out, Print("─".repeat(width as usize)))?;

        execute!(out, MoveTo(0, height.saturating_sub(1)))?;
        execute!(
            out,
            SetBackgroundColor(Color::Rgb { r: 24, g: 24, b: 37 }),
            SetForegroundColor(Color::White)
        )?;

        let status = if let Some((ref msg, time)) = self.status_message {
            if time.elapsed() < Duration::from_secs(5) {
                format!(" Status: {} ", msg)
            } else {
                " Ready ".to_string()
            }
        } else {
            " Ready. Daemon and active mounts persist in background when closing. ".to_string()
        };

        let pad = (width as usize).saturating_sub(status.len());
        execute!(out, Print(format!("{}{}", status, " ".repeat(pad))), ResetColor)?;
        Ok(())
    }
}
