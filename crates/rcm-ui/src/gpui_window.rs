use gpui_kit::component::badge::Badge;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::*;
use rcm_core::{DaemonState, Id, MountPreset, MountProfile, MountTarget};
use rcm_rc::types::ProviderInfo;
use rcm_ui_kit::view_model::{FileEntryViewModel, MountItemViewModel, RemoteItemViewModel, ServeItemViewModel, SidebarDestination};
use crate::app::AppController;

pub struct WizardModalData<'a> {
    pub step: usize,
    pub name: &'a str,
    pub selected_backend: &'a str,
    pub question_text: &'a str,
    pub question_help: &'a str,
    pub is_password: bool,
    pub oauth_url: Option<&'a str>,
    pub error: Option<&'a str>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    NewRemoteWizard {
        step: usize,
        name: String,
        selected_backend: String,
        question_text: String,
        question_help: String,
        is_password: bool,
        oauth_url: Option<String>,
        error: Option<String>,
    },
    NewMountModal {
        name: String,
        selected_remote: String,
        drive_letter: char,
        preset: MountPreset,
    },
    NewServeModal {
        name: String,
        selected_remote: String,
        protocol: rcm_core::ServeProtocol,
        addr: String,
    },
}

pub struct RcmDesktopWindow {
    app: AppController,
    current_destination: SidebarDestination,
    daemon_state: DaemonState,
    status_text: String,
    remotes: Vec<RemoteItemViewModel>,
    mounts: Vec<MountItemViewModel>,
    serves: Vec<ServeItemViewModel>,
    active_modal: ActiveModal,
    available_providers: Vec<ProviderInfo>,
    explorer_remote: Option<String>,
    explorer_path: String,
    explorer_files: Vec<FileEntryViewModel>,
    snapshots: Vec<rcm_core::SnapshotMeta>,

    // Real GPUI Interactive Input Entities
    wizard_name_input: Entity<InputState>,
    wizard_search_input: Entity<InputState>,
    wizard_answer_input: Entity<InputState>,
    mount_name_input: Entity<InputState>,
    mount_subpath_input: Entity<InputState>,
    serve_name_input: Entity<InputState>,
    serve_addr_input: Entity<InputState>,
    serve_user_input: Entity<InputState>,
    serve_pass_input: Entity<InputState>,
    wizard_driver: Option<rcm_rc::WizardDriver>,
}

impl RcmDesktopWindow {
    pub fn new(window: &mut Window, app: AppController, cx: &mut Context<Self>) -> Self {
        let wizard_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("e.g. my_gdrive"));
        let wizard_search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search providers (e.g. drive, s3, dropbox)..."));
        let wizard_answer_input = cx.new(|cx| InputState::new(window, cx).placeholder("Enter configuration value..."));
        let mount_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Profile name (e.g. Work Drive)"));
        let mount_subpath_input = cx.new(|cx| InputState::new(window, cx).placeholder("Subpath within remote (optional)"));
        let serve_name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Endpoint name (e.g. HomeWebDAV)"));
        let serve_addr_input = cx.new(|cx| InputState::new(window, cx).placeholder("127.0.0.1:8080"));
        let serve_user_input = cx.new(|cx| InputState::new(window, cx).placeholder("Username (required for LAN)"));
        let serve_pass_input = cx.new(|cx| InputState::new(window, cx).placeholder("Password (required for LAN)"));

        Self {
            app,
            current_destination: SidebarDestination::Home,
            daemon_state: DaemonState::Stopped,
            status_text: "Initializing Rclone Manager...".to_string(),
            remotes: Vec::new(),
            mounts: Vec::new(),
            serves: Vec::new(),
            active_modal: ActiveModal::None,
            available_providers: Vec::new(),
            explorer_remote: None,
            explorer_path: String::new(),
            explorer_files: Vec::new(),
            snapshots: Vec::new(),

            wizard_name_input,
            wizard_search_input,
            wizard_answer_input,
            mount_name_input,
            mount_subpath_input,
            serve_name_input,
            serve_addr_input,
            serve_user_input,
            serve_pass_input,
            wizard_driver: None,
        }
    }

    pub fn set_destination(&mut self, dest: SidebarDestination, cx: &mut Context<Self>) {
        self.current_destination = dest;
        cx.notify();
    }

    pub fn close_modal(&mut self, cx: &mut Context<Self>) {
        self.active_modal = ActiveModal::None;
        self.wizard_driver = None;
        cx.notify();
    }

    pub fn refresh_state(&mut self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        cx.spawn(async move |this, cx| {
            let (daemon_state, remotes, mounts, serves) = app.fetch_ui_data().await;
            let providers = app.fetch_providers().await.unwrap_or_default();
            let snaps = app.fetch_snapshots().await.unwrap_or_default();

            let _ = this.update(cx, |this, cx| {
                this.daemon_state = daemon_state;
                this.remotes = remotes;
                this.mounts = mounts;
                this.serves = serves;
                this.snapshots = snaps;
                if !providers.is_empty() {
                    this.available_providers = providers;
                }
                this.status_text = "Ready. Daemon running in background.".to_string();
                cx.notify();
            });
        })
        .detach();
    }

    pub fn trigger_reconcile(&mut self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        self.status_text = "Reconciling mounts and serves...".to_string();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _ = app.trigger_reconcile().await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn trigger_restart_daemon(&mut self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        self.status_text = "Restarting rclone daemon...".to_string();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let _ = app.trigger_restart_daemon().await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn open_mount_drive(&mut self, remote: String, target: String, preset_str: String, cx: &mut Context<Self>) {
        let app = self.app.clone();
        let preset = match preset_str.as_str() {
            "Streaming" => MountPreset::Streaming,
            "OfflineFirst" => MountPreset::OfflineFirst,
            "MaxCompatibility" => MountPreset::MaxCompatibility,
            "ReadOnly" => MountPreset::ReadOnly,
            _ => MountPreset::Balanced,
        };

        self.status_text = format!("Mounting {} to {}...", remote, target);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = app.mount_drive(&remote, &target, preset).await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => this.status_text = format!("Successfully mounted {} on {}", remote, target),
                    Err(e) => this.status_text = format!("Mount failed: {}", e),
                }
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn open_unmount_drive(&mut self, target: String, cx: &mut Context<Self>) {
        let app = self.app.clone();
        self.status_text = format!("Unmounting {}...", target);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = app.unmount_drive(&target).await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => this.status_text = format!("Successfully unmounted {}", target),
                    Err(e) => this.status_text = format!("Unmount failed: {}", e),
                }
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn open_path_in_explorer(&mut self, target: String, cx: &mut Context<Self>) {
        let clean_path = format!("{}\\", target.trim_end_matches('\\'));
        let _ = rcm_platform::opener::open_path_in_file_manager(&clean_path);
        self.status_text = format!("Opened {} in Windows Explorer", clean_path);
        cx.notify();
    }

    pub fn delete_mount_profile_action(&mut self, id: Id, cx: &mut Context<Self>) {
        let app = self.app.clone();
        cx.spawn(async move |this, cx| {
            let _ = app.delete_mount_profile(&id).await;
            let _ = this.update(cx, |this, cx| {
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn delete_remote_action(&mut self, name: String, cx: &mut Context<Self>) {
        let app = self.app.clone();
        self.status_text = format!("Deleting remote '{}'...", name);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = app.delete_remote(&name).await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => this.status_text = format!("Deleted remote '{}'", name),
                    Err(e) => this.status_text = format!("Delete failed: {}", e),
                }
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn browse_remote(&mut self, remote: String, cx: &mut Context<Self>) {
        self.explorer_remote = Some(remote.clone());
        self.explorer_path = String::new();
        self.current_destination = SidebarDestination::Files;
        self.status_text = format!("Loading directory listing for {}:...", remote);
        cx.notify();

        let app = self.app.clone();
        let rem = remote.clone();
        cx.spawn(async move |this, cx| {
            let res = app.list_files(&format!("{}:", rem), "").await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(files) = res {
                    this.explorer_files = files;
                    this.status_text = format!("Loaded {} files from {}", this.explorer_files.len(), remote);
                } else {
                    this.status_text = format!("Failed to list files for {}", remote);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn navigate_folder(&mut self, subpath: String, cx: &mut Context<Self>) {
        let rem = match &self.explorer_remote {
            Some(r) => r.clone(),
            None => return,
        };

        self.explorer_path = subpath.clone();
        self.status_text = format!("Entering {}...", subpath);
        cx.notify();

        let app = self.app.clone();
        cx.spawn(async move |this, cx| {
            let res = app.list_files(&format!("{}:", rem), &subpath).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(files) = res {
                    this.explorer_files = files;
                    this.status_text = format!("Loaded {} items in {}", this.explorer_files.len(), subpath);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn navigate_up(&mut self, cx: &mut Context<Self>) {
        let rem = match &self.explorer_remote {
            Some(r) => r.clone(),
            None => return,
        };

        let parent_path = if let Some(pos) = self.explorer_path.rfind('/') {
            self.explorer_path[..pos].to_string()
        } else {
            String::new()
        };

        self.explorer_path = parent_path.clone();
        let app = self.app.clone();
        cx.spawn(async move |this, cx| {
            let res = app.list_files(&format!("{}:", rem), &parent_path).await;
            let _ = this.update(cx, |this, cx| {
                if let Ok(files) = res {
                    this.explorer_files = files;
                    this.status_text = "Navigated up.".to_string();
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn open_new_serve_modal(&mut self, cx: &mut Context<Self>) {
        let default_remote = self.remotes.first().map(|r| r.name.clone()).unwrap_or_else(|| "remote".to_string());
        self.active_modal = ActiveModal::NewServeModal {
            name: format!("{}_webdav", default_remote),
            selected_remote: default_remote,
            protocol: rcm_core::ServeProtocol::Webdav,
            addr: "127.0.0.1:8080".to_string(),
        };
        cx.notify();
    }

    pub fn stop_serve_action(&mut self, id: u64, cx: &mut Context<Self>) {
        let app = self.app.clone();
        self.status_text = format!("Stopping serve #{}...", id);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = app.stop_serve(id).await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => this.status_text = format!("Stopped serve #{}", id),
                    Err(e) => this.status_text = format!("Stop serve failed: {}", e),
                }
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn restore_snapshot_action(&mut self, filename: String, cx: &mut Context<Self>) {
        let app = self.app.clone();
        self.status_text = format!("Restoring snapshot '{}'...", filename);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = app.restore_snapshot(&filename).await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(_) => this.status_text = format!("Restored snapshot '{}'", filename),
                    Err(e) => this.status_text = format!("Restore failed: {}", e),
                }
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    pub fn update_rclone_action(&mut self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        self.status_text = "Downloading latest official verified rclone...".to_string();
        cx.notify();

        cx.spawn(async move |this, cx| {
            let res = app.update_rclone().await;
            let _ = this.update(cx, |this, cx| {
                match res {
                    Ok(ver) => this.status_text = format!("Updated rclone to {}", ver),
                    Err(e) => this.status_text = format!("Update failed: {}", e),
                }
                this.refresh_state(cx);
            });
        })
        .detach();
    }

    // Modal dialog triggers
    pub fn open_new_mount_modal(&mut self, cx: &mut Context<Self>) {
        let default_remote = self.remotes.first().map(|r| r.name.clone()).unwrap_or_else(|| "remote".to_string());
        self.active_modal = ActiveModal::NewMountModal {
            name: format!("{} Drive", default_remote),
            selected_remote: default_remote,
            drive_letter: 'G',
            preset: MountPreset::Balanced,
        };
        cx.notify();
    }

    pub fn open_new_remote_wizard(&mut self, cx: &mut Context<Self>) {
        self.active_modal = ActiveModal::NewRemoteWizard {
            step: 0,
            name: "my_remote".to_string(),
            selected_backend: "drive".to_string(),
            question_text: String::new(),
            question_help: String::new(),
            is_password: false,
            oauth_url: None,
            error: None,
        };
        self.wizard_driver = None;
        cx.notify();
    }

    pub fn submit_wizard_answer(&mut self, answer: String, cx: &mut Context<Self>) {
        if let Some(ref mut driver) = self.wizard_driver {
            let is_pass = match &self.active_modal {
                ActiveModal::NewRemoteWizard { is_password, .. } => *is_password,
                _ => false,
            };

            let mut driver_clone = driver.clone();

            cx.spawn(async move |this, cx| {
                let res = driver_clone.answer(&answer, is_pass).await;
                let _ = this.update(cx, |this, cx| {
                    this.wizard_driver = Some(driver_clone);
                    match res {
                        Ok(rcm_rc::WizardStep::AskQuestion { state, option, error }) => {
                            if let ActiveModal::NewRemoteWizard { question_text, question_help, is_password, error: err_slot, .. } = &mut this.active_modal {
                                *question_text = state;
                                *question_help = option.help;
                                *is_password = option.is_password;
                                *err_slot = error;
                            }
                        }
                        Ok(rcm_rc::WizardStep::OAuthInProgress { auth_url }) => {
                            if let ActiveModal::NewRemoteWizard { question_text, question_help, oauth_url, .. } = &mut this.active_modal {
                                *question_text = "OAuth Authorization".to_string();
                                *question_help = "Please authorize in your web browser.".to_string();
                                *oauth_url = auth_url;
                            }
                        }
                        Ok(rcm_rc::WizardStep::Completed { .. }) => {
                            this.close_modal(cx);
                            this.refresh_state(cx);
                        }
                        Err(e) => {
                            if let ActiveModal::NewRemoteWizard { error, .. } = &mut this.active_modal {
                                *error = Some(e.to_string());
                            }
                        }
                        _ => {}
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

impl Render for RcmDesktopWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_dest = self.current_destination;
        let daemon_state = self.daemon_state.clone();
        let status_text = self.status_text.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x181825))
            .text_color(rgb(0xcdd6f4))
            .relative()
            // 1. Header Bar
            .child(self.render_header(&daemon_state, cx))
            // 2. Main Content (Sidebar + Active View)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_grow(1.0)
                    .w_full()
                    // Sidebar
                    .child(self.render_sidebar(current_dest, cx))
                    // Content Body
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_grow(1.0)
                            .p_6()
                            .overflow_hidden()
                            .child(match current_dest {
                                SidebarDestination::Home => self.render_home_view(cx).into_any_element(),
                                SidebarDestination::Remotes => self.render_remotes_view(cx).into_any_element(),
                                SidebarDestination::Mounts => self.render_mounts_view(cx).into_any_element(),
                                SidebarDestination::Serves => self.render_serves_view(cx).into_any_element(),
                                SidebarDestination::Files => self.render_files_view(cx).into_any_element(),
                                SidebarDestination::Settings => self.render_settings_view(cx).into_any_element(),
                            }),
                    ),
            )
            // 3. Footer Status Bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .h(px(32.0))
                    .px_4()
                    .bg(rgb(0x11111b))
                    .border_t_1()
                    .border_color(rgb(0x313244))
                    .text_size(px(12.0))
                    .text_color(rgb(0xa6adc8))
                    .child(div().child(status_text))
                    .child(div().child("Rclone Manager (Hardware Accelerated)")),
            )
            // 4. Modal Overlays
            .children(match &self.active_modal {
                ActiveModal::None => None,
                ActiveModal::NewMountModal { name, selected_remote, drive_letter, preset } => {
                    Some(self.render_new_mount_modal(name, selected_remote, *drive_letter, *preset, cx).into_any_element())
                }
                ActiveModal::NewServeModal { name, selected_remote, protocol, addr } => {
                    Some(self.render_new_serve_modal(name, selected_remote, protocol.clone(), addr, cx).into_any_element())
                }
                ActiveModal::NewRemoteWizard { step, name, selected_backend, question_text, question_help, is_password, oauth_url, error } => {
                    let data = WizardModalData {
                        step: *step,
                        name,
                        selected_backend,
                        question_text,
                        question_help,
                        is_password: *is_password,
                        oauth_url: oauth_url.as_deref(),
                        error: error.as_deref(),
                    };
                    Some(self.render_wizard_modal(&data, cx).into_any_element())
                }
            })
    }
}

impl RcmDesktopWindow {
    fn render_header(&self, daemon_state: &DaemonState, cx: &mut Context<Self>) -> impl IntoElement {
        let (dot_color, state_label, detail_label) = match daemon_state {
            DaemonState::Ready { version, pid, addr, .. } => (
                rgb(0xa6e3a1), // Green
                format!("rclone {} [READY]", version),
                format!("PID {} • {}", pid, addr),
            ),
            DaemonState::Starting => (
                rgb(0xf9e2af), // Amber
                "rclone [STARTING...]".to_string(),
                "Launching daemon".to_string(),
            ),
            DaemonState::Stopped => (
                rgb(0xf38ba8), // Red
                "rclone [STOPPED]".to_string(),
                "Daemon is stopped".to_string(),
            ),
            DaemonState::Degraded { reason } => (
                rgb(0xf9e2af),
                "rclone [DEGRADED]".to_string(),
                reason.clone(),
            ),
            DaemonState::Crashed { last_error, .. } => (
                rgb(0xf38ba8),
                "rclone [CRASHED]".to_string(),
                last_error.clone(),
            ),
            DaemonState::Stopping => (
                rgb(0xf9e2af),
                "rclone [STOPPING...]".to_string(),
                "Graceful unmount".to_string(),
            ),
            DaemonState::Failed { reason } => (
                rgb(0xf38ba8),
                "rclone [FAILED]".to_string(),
                reason.clone(),
            ),
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(48.0))
            .px_4()
            .bg(rgb(0x1e1e2e))
            .border_b_1()
            .border_color(rgb(0x313244))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0x89b4fa))
                            .child("Rclone Manager"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x313244))
                            .child(div().text_color(dot_color).child("●"))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(state_label),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(0xa6adc8))
                                    .child(detail_label),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("refresh_btn")
                            .label("↻ Refresh")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_state(cx);
                            })),
                    ),
            )
    }

    fn render_sidebar(&self, current: SidebarDestination, cx: &mut Context<Self>) -> impl IntoElement {
        let items = [
            (SidebarDestination::Home, "Dashboard", "📊"),
            (SidebarDestination::Remotes, "Remotes", "☁️"),
            (SidebarDestination::Mounts, "Mounts", "💽"),
            (SidebarDestination::Serves, "Serves", "🌐"),
            (SidebarDestination::Files, "Files", "📁"),
            (SidebarDestination::Settings, "Settings", "⚙️"),
        ];

        div()
            .flex()
            .flex_col()
            .w(px(200.0))
            .h_full()
            .bg(rgb(0x181825))
            .border_r_1()
            .border_color(rgb(0x313244))
            .p_2()
            .gap_1()
            .children(items.into_iter().map(|(dest, label, icon)| {
                let is_active = dest == current;
                let btn_label = if is_active {
                    format!("{} > {} <", icon, label)
                } else {
                    format!("{}   {}", icon, label)
                };

                div()
                    .w_full()
                    .child(
                        Button::new(format!("nav_{:?}", dest))
                            .label(btn_label)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_destination(dest, cx);
                            })),
                    )
            }))
    }

    fn render_home_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("Dashboard"))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(0xa6adc8))
                            .child("Overview of rclone background daemon, active drive letter mounts, and network servers."),
                    ),
            )
            // Stat Cards Grid
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .child(self.stat_card("Active Mounts", &self.mounts.iter().filter(|m| m.is_mounted).count().to_string(), "Virtual drives mounted", rgb(0x89b4fa)))
                    .child(self.stat_card("Active Serves", &self.serves.iter().filter(|s| s.is_running).count().to_string(), "WebDAV / SFTP endpoints", rgb(0xa6e3a1)))
                    .child(self.stat_card("Configured Remotes", &self.remotes.len().to_string(), "Cloud providers linked", rgb(0xf9e2af))),
            )
            // Action Box
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x1e1e2e))
                    .border_1()
                    .border_color(rgb(0x313244))
                    .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Quick Actions"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_3()
                            .child(
                                Button::new("mount_cloud_btn")
                                    .label("💽 + New Mount")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_new_mount_modal(cx);
                                    })),
                            )
                            .child(
                                Button::new("nav_remotes_btn")
                                    .label("☁ + Add Remote")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_new_remote_wizard(cx);
                                    })),
                            )
                            .child(
                                Button::new("sync_mounts_btn")
                                    .label("⚡ Reconcile Mounts")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.trigger_reconcile(cx);
                                    })),
                            ),
                    ),
            )
    }

    fn stat_card(&self, title: &str, value: &str, subtitle: &str, color: Rgba) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(220.0))
            .p_4()
            .rounded_lg()
            .bg(rgb(0x1e1e2e))
            .border_1()
            .border_color(rgb(0x313244))
            .child(div().text_size(px(12.0)).text_color(rgb(0xa6adc8)).child(title.to_string()))
            .child(div().text_size(px(28.0)).font_weight(FontWeight::BOLD).text_color(color).child(value.to_string()))
            .child(div().text_size(px(11.0)).text_color(rgb(0x6c7086)).child(subtitle.to_string()))
    }

    fn render_remotes_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("Cloud Remotes"))
                            .child(div().text_size(px(13.0)).text_color(rgb(0xa6adc8)).child("Configured backends in rclone.conf (managed through safe RC calls).")),
                    )
                    .child(
                        Button::new("add_remote_btn")
                            .label("+ New Remote Wizard")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_new_remote_wizard(cx);
                            })),
                    ),
            )
            .child(
                if self.remotes.is_empty() {
                    div()
                        .p_8()
                        .rounded_lg()
                        .bg(rgb(0x1e1e2e))
                        .text_color(rgb(0xa6adc8))
                        .child("No remotes configured yet. Click '+ New Remote Wizard' to link Google Drive, S3, Dropbox, etc.")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(self.remotes.iter().map(|r| {
                            let rem_name = r.name.clone();
                            let browse_name = r.name.clone();
                            let del_name = r.name.clone();

                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .p_3()
                                .rounded_md()
                                .bg(rgb(0x1e1e2e))
                                .border_1()
                                .border_color(rgb(0x313244))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_3()
                                        .child(div().text_size(px(16.0)).child("☁"))
                                        .child(div().font_weight(FontWeight::SEMIBOLD).child(rem_name.clone()))
                                        .child(Badge::new().child("cloud")),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_2()
                                        .child(
                                            Button::new(format!("browse_{}", rem_name))
                                                .label("📁 Browse")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.browse_remote(browse_name.clone(), cx);
                                                })),
                                        )
                                        .child(
                                            Button::new(format!("del_{}", rem_name))
                                                .label("🗑 Delete")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.delete_remote_action(del_name.clone(), cx);
                                                })),
                                        ),
                                )
                        }))
                },
            )
    }

    fn render_mounts_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("Mounts & Drives"))
                            .child(div().text_size(px(13.0)).text_color(rgb(0xa6adc8)).child("Mount cloud remotes as native Windows drive letters using WinFsp.")),
                    )
                    .child(
                        Button::new("add_mount_btn")
                            .label("+ New Mount Profile")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_new_mount_modal(cx);
                            })),
                    ),
            )
            .child(
                if self.mounts.is_empty() {
                    div()
                        .p_8()
                        .rounded_lg()
                        .bg(rgb(0x1e1e2e))
                        .text_color(rgb(0xa6adc8))
                        .child("No mount profiles found. Click '+ New Mount Profile' to mount a remote as drive letter G:, M:, etc.")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(self.mounts.iter().map(|m| {
                            let mount_remote = m.remote.clone();
                            let mount_target = m.target.clone();
                            let unmount_target = m.target.clone();
                            let open_target = m.target.clone();
                            let preset_str = m.preset.clone();
                            let is_mounted = m.is_mounted;
                            let profile_id = m.id.clone();

                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .p_3()
                                .rounded_md()
                                .bg(rgb(0x1e1e2e))
                                .border_1()
                                .border_color(rgb(0x313244))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_4()
                                        .child(
                                            div()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(if is_mounted { rgb(0xa6e3a1) } else { rgb(0x89b4fa) })
                                                .child(m.target.clone()),
                                        )
                                        .child(div().child(format!("{} ({})", m.name, m.remote)))
                                        .child(Badge::new().child(m.preset.clone()))
                                        .child(
                                            Badge::new().child(if is_mounted { "● Mounted" } else { "○ Stopped" }),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_2()
                                        .children(if is_mounted {
                                            vec![
                                                Button::new(format!("open_{}", m.name))
                                                    .label("📂 Open")
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.open_path_in_explorer(open_target.clone(), cx);
                                                    }))
                                                    .into_any_element(),
                                                Button::new(format!("unmnt_{}", m.name))
                                                    .label("⏹ Unmount")
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.open_unmount_drive(unmount_target.clone(), cx);
                                                    }))
                                                    .into_any_element(),
                                            ]
                                        } else {
                                            vec![
                                                Button::new(format!("mnt_{}", m.name))
                                                    .label("▶ Mount")
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.open_mount_drive(mount_remote.clone(), mount_target.clone(), preset_str.clone(), cx);
                                                    }))
                                                    .into_any_element(),
                                                Button::new(format!("del_mnt_{}", m.name))
                                                    .label("🗑 Delete")
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.delete_mount_profile_action(profile_id.clone(), cx);
                                                    }))
                                                    .into_any_element(),
                                            ]
                                        }),
                                )
                        }))
                },
            )
    }

    fn render_serves_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("Network Serves"))
                            .child(div().text_size(px(13.0)).text_color(rgb(0xa6adc8)).child("Expose cloud storage via WebDAV, SFTP, HTTP, or S3 gateway.")),
                    )
                    .child(
                        Button::new("add_serve_btn")
                            .label("+ New Serve Profile")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_new_serve_modal(cx);
                            })),
                    ),
            )
            .child(
                if self.serves.is_empty() {
                    div()
                        .p_8()
                        .rounded_lg()
                        .bg(rgb(0x1e1e2e))
                        .text_color(rgb(0xa6adc8))
                        .child("No active network serves configured. Click '+ New Serve Profile' to share cloud files over LAN.")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(self.serves.iter().map(|s| {
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .p_3()
                                .rounded_md()
                                .bg(rgb(0x1e1e2e))
                                .border_1()
                                .border_color(rgb(0x313244))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_3()
                                        .child(Badge::new().child(s.protocol.clone()))
                                        .child(div().font_weight(FontWeight::MEDIUM).child(format!("{} on {}", s.name, s.addr)))
                                        .child(div().text_color(rgb(0x89b4fa)).text_size(px(12.0)).child(s.url.clone())),
                                )
                        }))
                },
            )
    }

    fn render_files_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_rem = self.explorer_remote.clone();
        let current_path = self.explorer_path.clone();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("File Explorer"))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(0xa6adc8))
                                    .child(match &current_rem {
                                        Some(r) => format!("Browsing {}:{}/", r, current_path),
                                        None => "Select a remote below to explore files.".to_string(),
                                    }),
                            ),
                    )
                    .children(if !current_path.is_empty() {
                        Some(
                            Button::new("up_folder_btn")
                                .label("⬆ Up to Parent")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_up(cx);
                                })),
                        )
                    } else {
                        None
                    }),
            )
            // Remote Selector Row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .children(self.remotes.iter().map(|r| {
                        let name = r.name.clone();
                        let is_sel = current_rem.as_deref() == Some(&name);
                        Button::new(format!("sel_{}", name))
                            .label(if is_sel { format!("▶ {}", name) } else { name.clone() })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.browse_remote(name.clone(), cx);
                            }))
                    })),
            )
            // Files Table
            .child(
                if self.explorer_files.is_empty() {
                    div()
                        .p_8()
                        .rounded_lg()
                        .bg(rgb(0x1e1e2e))
                        .text_color(rgb(0xa6adc8))
                        .child(if current_rem.is_some() {
                            "Folder is empty or directory listing in progress..."
                        } else {
                            "Select a remote above to list its contents."
                        })
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .max_h(px(400.0))
                        .overflow_hidden()
                        .children(self.explorer_files.iter().map(|f| {
                            let icon = if f.is_dir { "📁" } else { "📄" };
                            let path = f.path.clone();
                            let is_dir = f.is_dir;
                            let size_display = if f.is_dir {
                                "DIR".to_string()
                            } else {
                                format!("{:.1} KB", f.size_bytes as f64 / 1024.0)
                            };

                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .bg(rgb(0x1e1e2e))
                                .border_1()
                                .border_color(rgb(0x313244))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_3()
                                        .child(div().child(icon))
                                        .child(
                                            if is_dir {
                                                Button::new(format!("dir_{}", f.name))
                                                    .label(format!("{} ↗", f.name))
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.navigate_folder(path.clone(), cx);
                                                    }))
                                                    .into_any_element()
                                            } else {
                                                div().font_weight(FontWeight::MEDIUM).child(f.name.clone()).into_any_element()
                                            },
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_4()
                                        .child(div().text_color(rgb(0xa6adc8)).text_size(px(12.0)).child(size_display))
                                        .child(div().text_color(rgb(0x6c7086)).text_size(px(11.0)).child(f.mod_time.clone())),
                                )
                        }))
                },
            )
    }

    fn render_settings_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("Settings & Controls"))
                    .child(div().text_size(px(13.0)).text_color(rgb(0xa6adc8)).child("Daemon supervision, automatic backups, and rclone binary updates.")),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x1e1e2e))
                    .border_1()
                    .border_color(rgb(0x313244))
                    .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Daemon Lifecycle"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_3()
                            .child(
                                Button::new("restart_daemon_btn")
                                    .label("↻ Restart Daemon")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.trigger_restart_daemon(cx);
                                    })),
                            )
                            .child(
                                Button::new("sync_all_btn")
                                    .label("⚡ Force Reconcile")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.trigger_reconcile(cx);
                                    })),
                            )
                            .child(
                                Button::new("update_rclone_btn")
                                    .label("⬇ Update rclone Binary")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.update_rclone_action(cx);
                                    })),
                            ),
                    ),
            )
            // Snapshots & Backups List
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x1e1e2e))
                    .border_1()
                    .border_color(rgb(0x313244))
                    .child(div().text_size(px(15.0)).font_weight(FontWeight::SEMIBOLD).child("Configuration Snapshots (Automatic Rollback)"))
                    .child(
                        if self.snapshots.is_empty() {
                            div().text_size(px(12.0)).text_color(rgb(0xa6adc8)).child("No previous snapshots stored yet.")
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .max_h(px(240.0))
                                .overflow_hidden()
                                .children(self.snapshots.iter().rev().take(10).map(|snap| {
                                    let filename = snap.filename.clone();
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .p_2()
                                        .rounded_md()
                                        .bg(rgb(0x181825))
                                        .border_1()
                                        .border_color(rgb(0x313244))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap_3()
                                                .child(Badge::new().child(format!("{:?}", snap.reason)))
                                                .child(div().text_size(px(12.0)).child(format!("{} ({} bytes)", snap.filename, snap.file_size_bytes))),
                                        )
                                        .child(
                                            Button::new(format!("rst_{}", snap.filename))
                                                .label("↻ Restore")
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.restore_snapshot_action(filename.clone(), cx);
                                                })),
                                        )
                                }))
                        },
                    ),
            )
    }

    // Modal: New Mount Profile Dialog
    fn render_new_mount_modal(
        &self,
        _name: &str,
        selected_remote: &str,
        drive_letter: char,
        preset: MountPreset,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let preset_str = match preset {
            MountPreset::Balanced => "Balanced",
            MountPreset::Streaming => "Streaming",
            MountPreset::OfflineFirst => "OfflineFirst",
            MountPreset::MaxCompatibility => "MaxCompatibility",
            _ => "Balanced",
        };

        let remotes_list = self.remotes.clone();
        let cur_remote = selected_remote.to_string();

        div()
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .bg(rgba(0x000000cc))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(580.0))
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(0x1e1e2e))
                    .border_1()
                    .border_color(rgb(0x45475a))
                    .gap_4()
                    .child(div().text_size(px(18.0)).font_weight(FontWeight::BOLD).child("Create New Mount Profile"))
                    // Mount Profile Name Input
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Profile Name:"))
                            .child(Input::new(&self.mount_name_input)),
                    )
                    // Select Cloud Remote
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Select Cloud Remote:"))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .children(remotes_list.iter().map(|r| {
                                        let r_name = r.name.clone();
                                        let is_sel = r_name == cur_remote;
                                        Button::new(format!("pick_rem_{}", r_name))
                                            .label(if is_sel { format!("● {}", r_name) } else { r_name.clone() })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let ActiveModal::NewMountModal { selected_remote, .. } = &mut this.active_modal {
                                                    *selected_remote = r_name.clone();
                                                }
                                                cx.notify();
                                            }))
                                    })),
                            ),
                    )
                    // Mount Subpath Input
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Subpath in Remote (optional):"))
                            .child(Input::new(&self.mount_subpath_input)),
                    )
                    // Target Drive Letter
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Target Drive Letter:"))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .children(['G', 'M', 'Z', 'X', 'Y', 'P'].into_iter().map(|letter| {
                                        let is_sel = letter == drive_letter;
                                        Button::new(format!("drive_{}", letter))
                                            .label(if is_sel { format!("● {}:", letter) } else { format!("{}:", letter) })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let ActiveModal::NewMountModal { drive_letter, .. } = &mut this.active_modal {
                                                    *drive_letter = letter;
                                                }
                                                cx.notify();
                                            }))
                                    })),
                            ),
                    )
                    // Mount Preset Buttons
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child(format!("Mount Preset (Current: {}):", preset_str)))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        Button::new("p_balanced")
                                            .label("Balanced (writes cache)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let ActiveModal::NewMountModal { preset, .. } = &mut this.active_modal {
                                                    *preset = MountPreset::Balanced;
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("p_stream")
                                            .label("Streaming (full cache)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let ActiveModal::NewMountModal { preset, .. } = &mut this.active_modal {
                                                    *preset = MountPreset::Streaming;
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    // Footer Buttons
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .pt_4()
                            .border_t_1()
                            .border_color(rgb(0x313244))
                            .child(
                                Button::new("cancel_mount_modal")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_modal(cx);
                                    })),
                            )
                            .child(
                                Button::new("confirm_save_mount")
                                    .label(format!("Save & Mount as {}:", drive_letter))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let app = this.app.clone();
                                        let typed_name = this.mount_name_input.read(cx).value().to_string();
                                        let typed_subpath = this.mount_subpath_input.read(cx).value().to_string();

                                        let full_remote = if typed_subpath.is_empty() {
                                            cur_remote.clone()
                                        } else {
                                            format!("{}:{}", cur_remote, typed_subpath.trim_start_matches('/'))
                                        };

                                        let final_name = if typed_name.is_empty() {
                                            format!("{} ({}:)", cur_remote, drive_letter)
                                        } else {
                                            typed_name
                                        };

                                        let profile = MountProfile::new(
                                            final_name,
                                            full_remote,
                                            MountTarget::DriveLetter(drive_letter),
                                        );

                                        cx.spawn(async move |this, cx| {
                                            let _ = app.save_mount_profile(profile).await;
                                            let _ = this.update(cx, |this, cx| {
                                                this.close_modal(cx);
                                                this.refresh_state(cx);
                                            });
                                        })
                                        .detach();
                                    })),
                            ),
                    ),
            )
    }

    // Modal: New Serve Profile Dialog (FR-SV-01, FR-SV-03, FR-SV-05)
    fn render_new_serve_modal(
        &self,
        _name: &str,
        selected_remote: &str,
        protocol: rcm_core::ServeProtocol,
        _addr: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let cur_remote = selected_remote.to_string();
        let cur_proto = protocol.clone();
        let remotes_list = self.remotes.clone();

        div()
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .bg(rgba(0x000000cc))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(580.0))
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(0x1e1e2e))
                    .border_1()
                    .border_color(rgb(0x45475a))
                    .gap_4()
                    .child(div().text_size(px(18.0)).font_weight(FontWeight::BOLD).child("Create New Serve Endpoint"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Endpoint Name:"))
                            .child(Input::new(&self.serve_name_input)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Select Cloud Remote:"))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .children(remotes_list.iter().map(|r| {
                                        let r_name = r.name.clone();
                                        let is_sel = r_name == cur_remote;
                                        Button::new(format!("pick_srv_rem_{}", r_name))
                                            .label(if is_sel { format!("● {}", r_name) } else { r_name.clone() })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let ActiveModal::NewServeModal { selected_remote, .. } = &mut this.active_modal {
                                                    *selected_remote = r_name.clone();
                                                }
                                                cx.notify();
                                            }))
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child(format!("Protocol (Current: {}):", protocol)))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        Button::new("proto_webdav")
                                            .label("WebDAV")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let ActiveModal::NewServeModal { protocol, .. } = &mut this.active_modal {
                                                    *protocol = rcm_core::ServeProtocol::Webdav;
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("proto_sftp")
                                            .label("SFTP")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let ActiveModal::NewServeModal { protocol, .. } = &mut this.active_modal {
                                                    *protocol = rcm_core::ServeProtocol::Sftp;
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("proto_http")
                                            .label("HTTP")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let ActiveModal::NewServeModal { protocol, .. } = &mut this.active_modal {
                                                    *protocol = rcm_core::ServeProtocol::Http;
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Bind Address & Port:"))
                            .child(Input::new(&self.serve_addr_input)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_grow(1.0)
                                    .gap_1()
                                    .child(div().text_size(px(12.0)).text_color(rgb(0xa6adc8)).child("Username (for LAN):"))
                                    .child(Input::new(&self.serve_user_input)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_grow(1.0)
                                    .gap_1()
                                    .child(div().text_size(px(12.0)).text_color(rgb(0xa6adc8)).child("Password (for LAN):"))
                                    .child(Input::new(&self.serve_pass_input).mask_toggle()),
                            ),
                    )
                    // Footer
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .pt_4()
                            .border_t_1()
                            .border_color(rgb(0x313244))
                            .child(
                                Button::new("cancel_serve_modal")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_modal(cx);
                                    })),
                            )
                            .child(
                                Button::new("confirm_start_serve")
                                    .label("Start Serve Endpoint")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let app = this.app.clone();
                                        let typed_name = this.serve_name_input.read(cx).value().to_string();
                                        let typed_addr = this.serve_addr_input.read(cx).value().to_string();
                                        let typed_user = this.serve_user_input.read(cx).value().to_string();
                                        let typed_pass = this.serve_pass_input.read(cx).value().to_string();

                                        let mut profile = rcm_core::ServeProfile::new(
                                            if typed_name.is_empty() { format!("{}_serve", cur_remote) } else { typed_name },
                                            cur_remote.clone(),
                                            cur_proto.clone(),
                                            if typed_addr.is_empty() { "127.0.0.1:8080".to_string() } else { typed_addr },
                                        );

                                        if !typed_user.is_empty() {
                                            profile.user = Some(typed_user);
                                        }
                                        if !typed_pass.is_empty() {
                                            profile.pass = Some(typed_pass);
                                        }

                                        cx.spawn(async move |this, cx| {
                                            let _ = app.start_serve(profile).await;
                                            let _ = this.update(cx, |this, cx| {
                                                this.close_modal(cx);
                                                this.refresh_state(cx);
                                            });
                                        })
                                        .detach();
                                    })),
                            ),
                    ),
            )
    }

    // Modal: New Remote Wizard (R2, CF-2, CF-3)
    fn render_wizard_modal(
        &self,
        data: &WizardModalData<'_>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let step = data.step;
        let cur_backend = data.selected_backend.to_string();
        let q_title = if data.question_text.is_empty() { "Authentication & Options".to_string() } else { data.question_text.to_string() };
        let q_help = if data.question_help.is_empty() { "Complete setup for this cloud backend.".to_string() } else { data.question_help.to_string() };
        let is_password = data.is_password;

        let popular_backends = [
            ("drive", "Google Drive", "Cloud storage by Google"),
            ("s3", "Amazon S3", "S3 compliant object storage"),
            ("dropbox", "Dropbox", "Dropbox cloud sync"),
            ("onedrive", "Microsoft OneDrive", "OneDrive personal / business"),
            ("webdav", "WebDAV", "Generic WebDAV server"),
            ("sftp", "SFTP", "SSH file transfer"),
            ("crypt", "Encrypt (Crypt)", "Client-side encryption wrapper"),
        ];

        let search_query = self.wizard_search_input.read(cx).value().to_string().to_lowercase();

        div()
            .absolute()
            .size_full()
            .top_0()
            .left_0()
            .bg(rgba(0x000000cc))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(660.0))
                    .max_h(px(580.0))
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(0x1e1e2e))
                    .border_1()
                    .border_color(rgb(0x45475a))
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(18.0))
                            .font_weight(FontWeight::BOLD)
                            .child(if step == 0 {
                                "New Remote Wizard — Select Cloud Backend (Step 1 of 2)"
                            } else {
                                "New Remote Wizard — Configure Backend (Step 2 of 2)"
                            }),
                    )
                    // Body Step 0 vs Step 1
                    .child(if step == 0 {
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            // Real Remote Name Text Input!
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Remote Name:"))
                                    .child(Input::new(&self.wizard_name_input)),
                            )
                            // Real Search Filter Input!
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Search Cloud Providers:"))
                                    .child(Input::new(&self.wizard_search_input)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .max_h(px(220.0))
                                    .overflow_hidden()
                                    .children(popular_backends.into_iter().filter(|(b_id, title, desc)| {
                                        if search_query.is_empty() {
                                            true
                                        } else {
                                            b_id.contains(&search_query) || title.to_lowercase().contains(&search_query) || desc.to_lowercase().contains(&search_query)
                                        }
                                    }).map(|(backend_id, title, desc)| {
                                        let is_sel = cur_backend == backend_id;
                                        let b_id = backend_id.to_string();

                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .p_2()
                                            .rounded_md()
                                            .bg(if is_sel { rgb(0x313244) } else { rgb(0x181825) })
                                            .border_1()
                                            .border_color(if is_sel { rgb(0x89b4fa) } else { rgb(0x313244) })
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                                                    .child(div().text_size(px(11.0)).text_color(rgb(0xa6adc8)).child(desc)),
                                            )
                                            .child(
                                                Button::new(format!("pick_b_{}", backend_id))
                                                    .label(if is_sel { "Selected ✓" } else { "Select" })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if let ActiveModal::NewRemoteWizard { selected_backend, .. } = &mut this.active_modal {
                                                            *selected_backend = b_id.clone();
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                    })),
                            )
                    } else {
                        // Step 1: Question or OAuth
                        div()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(q_title),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xa6adc8))
                                    .child(q_help),
                            )
                            // Real Interactive Answer Input Field!
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_size(px(12.0)).font_weight(FontWeight::MEDIUM).child("Your Value:"))
                                    .child(
                                        if is_password {
                                            Input::new(&self.wizard_answer_input).mask_toggle().into_any_element()
                                        } else {
                                            Input::new(&self.wizard_answer_input).into_any_element()
                                        },
                                    ),
                            )
                            .children(if let Some(url) = data.oauth_url {
                                let u = url.to_string();
                                vec![
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .p_3()
                                        .rounded_md()
                                        .bg(rgb(0x313244))
                                        .child(div().font_weight(FontWeight::SEMIBOLD).child("OAuth Browser Authentication Required"))
                                        .child(div().text_size(px(11.0)).child(format!("URL: {}", u)))
                                        .child(
                                            Button::new("open_oauth_btn")
                                                .label("🌐 Open Auth in Browser")
                                                .on_click(cx.listener(move |_, _, _, _| {
                                                    let _ = rcm_platform::opener::open_url_in_browser(&u);
                                                })),
                                        )
                                        .into_any_element(),
                                ]
                            } else {
                                Vec::new()
                            })
                            .children(if let Some(err) = data.error {
                                vec![
                                    div()
                                        .p_2()
                                        .rounded_md()
                                        .bg(rgb(0x311825))
                                        .text_color(rgb(0xf38ba8))
                                        .child(format!("Error: {}", err))
                                        .into_any_element(),
                                ]
                            } else {
                                Vec::new()
                            })
                    })
                    // Footer
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .pt_4()
                            .border_t_1()
                            .border_color(rgb(0x313244))
                            .child(
                                Button::new("cancel_wizard")
                                    .label("Cancel")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_modal(cx);
                                    })),
                            )
                            .child(if step == 0 {
                                Button::new("wizard_next_btn")
                                    .label("Continue to Configuration →")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let app = this.app.clone();
                                        let typed_name = this.wizard_name_input.read(cx).value().to_string();
                                        let n = if typed_name.is_empty() { "my_remote".to_string() } else { typed_name };
                                        let b = cur_backend.clone();

                                        cx.spawn(async move |this, cx| {
                                            // Start wizard via driver
                                            let state_arc = app.state();
                                            let s = state_arc.read().await;
                                            if let Some(ref rc) = s.rc_client {
                                                let mut driver = rcm_rc::WizardDriver::new(rc.clone(), &n, &b, std::collections::BTreeMap::new(), false);
                                                let step_res = driver.start().await;

                                                let _ = this.update(cx, |this, cx| {
                                                    this.wizard_driver = Some(driver);
                                                    match step_res {
                                                        Ok(rcm_rc::WizardStep::AskQuestion { state, option, error }) => {
                                                            this.active_modal = ActiveModal::NewRemoteWizard {
                                                                step: 1,
                                                                name: n,
                                                                selected_backend: b,
                                                                question_text: state,
                                                                question_help: option.help,
                                                                is_password: option.is_password,
                                                                oauth_url: None,
                                                                error,
                                                            };
                                                        }
                                                        Ok(rcm_rc::WizardStep::Completed { .. }) => {
                                                            this.close_modal(cx);
                                                            this.refresh_state(cx);
                                                        }
                                                        Ok(rcm_rc::WizardStep::OAuthInProgress { auth_url }) => {
                                                            this.active_modal = ActiveModal::NewRemoteWizard {
                                                                step: 1,
                                                                name: n,
                                                                selected_backend: b,
                                                                question_text: "OAuth Authorization".to_string(),
                                                                question_help: "Please complete authentication in your browser.".to_string(),
                                                                is_password: false,
                                                                oauth_url: auth_url,
                                                                error: None,
                                                            };
                                                        }
                                                        _ => {
                                                            this.close_modal(cx);
                                                        }
                                                    }
                                                    cx.notify();
                                                });
                                            }
                                        }).detach();
                                    }))
                            } else {
                                Button::new("wizard_answer_submit_btn")
                                    .label("Submit Answer →")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let typed_ans = this.wizard_answer_input.read(cx).value().to_string();
                                        this.submit_wizard_answer(typed_ans, cx);
                                    }))
                            }),
                    ),
            )
    }
}
