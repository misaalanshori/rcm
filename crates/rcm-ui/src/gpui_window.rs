use gpui_kit::component::badge::Badge;
use gpui_kit::component::button::Button;
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
        filter: String,
        question_text: String,
        question_help: String,
        answer_input: String,
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
    explorer_files: Vec<FileEntryViewModel>,
}

impl RcmDesktopWindow {
    pub fn new(app: AppController) -> Self {
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
            explorer_files: Vec::new(),
        }
    }

    pub fn set_destination(&mut self, dest: SidebarDestination, cx: &mut Context<Self>) {
        self.current_destination = dest;
        cx.notify();
    }

    pub fn close_modal(&mut self, cx: &mut Context<Self>) {
        self.active_modal = ActiveModal::None;
        cx.notify();
    }

    pub fn refresh_state(&mut self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        cx.spawn(async move |this, cx| {
            let (daemon_state, remotes, mounts, serves) = app.fetch_ui_data().await;
            let providers = app.fetch_providers().await.unwrap_or_default();

            let _ = this.update(cx, |this, cx| {
                this.daemon_state = daemon_state;
                this.remotes = remotes;
                this.mounts = mounts;
                this.serves = serves;
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
            filter: String::new(),
            question_text: String::new(),
            question_help: String::new(),
            answer_input: String::new(),
            is_password: false,
            oauth_url: None,
            error: None,
        };
        cx.notify();
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
                ActiveModal::NewRemoteWizard { step, name, selected_backend, filter: _, question_text, question_help, answer_input: _, is_password: _, oauth_url, error } => {
                    let data = WizardModalData {
                        step: *step,
                        name,
                        selected_backend,
                        question_text,
                        question_help,
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

    fn render_serves_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("Network Serves"))
                    .child(div().text_size(px(13.0)).text_color(rgb(0xa6adc8)).child("Expose cloud storage via WebDAV, SFTP, HTTP, or S3 gateway.")),
            )
            .child(
                if self.serves.is_empty() {
                    div()
                        .p_8()
                        .rounded_lg()
                        .bg(rgb(0x1e1e2e))
                        .text_color(rgb(0xa6adc8))
                        .child("No active network serves configured.")
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
                                        .gap_3()
                                        .child(Badge::new().child(s.protocol.clone()))
                                        .child(div().child(format!("{} on {}", s.name, s.addr))),
                                )
                        }))
                },
            )
    }

    fn render_files_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_rem = self.explorer_remote.clone();

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
                                        Some(r) => format!("Browsing files on {}:", r),
                                        None => "Select a remote below to explore files.".to_string(),
                                    }),
                            ),
                    ),
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
                                        .child(div().font_weight(FontWeight::MEDIUM).child(f.name.clone())),
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
                            ),
                    ),
            )
    }

    // Modal: New Mount Profile Dialog
    fn render_new_mount_modal(
        &self,
        name: &str,
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
        let cur_name = name.to_string();

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
                    .w(px(560.0))
                    .p_6()
                    .rounded_xl()
                    .bg(rgb(0x1e1e2e))
                    .border_1()
                    .border_color(rgb(0x45475a))
                    .gap_4()
                    .child(div().text_size(px(18.0)).font_weight(FontWeight::BOLD).child("Create New Mount Profile"))
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
                                        let profile = MountProfile::new(
                                            cur_name.clone(),
                                            cur_remote.clone(),
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

    // Modal: New Remote Wizard (R2, CF-2, CF-3)
    fn render_wizard_modal(
        &self,
        data: &WizardModalData<'_>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let step = data.step;
        let cur_name = data.name.to_string();
        let cur_backend = data.selected_backend.to_string();
        let q_title = if data.question_text.is_empty() { "Authentication & Options".to_string() } else { data.question_text.to_string() };
        let q_help = if data.question_help.is_empty() { "Complete setup for this cloud backend.".to_string() } else { data.question_help.to_string() };

        let popular_backends = [
            ("drive", "Google Drive", "Cloud storage by Google"),
            ("s3", "Amazon S3", "S3 compliant object storage"),
            ("dropbox", "Dropbox", "Dropbox cloud sync"),
            ("onedrive", "Microsoft OneDrive", "OneDrive personal / business"),
            ("webdav", "WebDAV", "Generic WebDAV server"),
            ("sftp", "SFTP", "SSH file transfer"),
            ("crypt", "Encrypt (Crypt)", "Client-side encryption wrapper"),
        ];

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
                    .w(px(640.0))
                    .max_h(px(560.0))
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
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Remote Name:"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_2()
                                            .children([
                                                ("my_gdrive", "my_gdrive"),
                                                ("my_s3", "my_s3"),
                                                ("my_dropbox", "my_dropbox"),
                                                ("my_backup", "my_backup"),
                                            ].into_iter().map(|(lbl, val)| {
                                                let is_sel = cur_name == val;
                                                Button::new(format!("name_{}", val))
                                                    .label(if is_sel { format!("● {}", lbl) } else { lbl.to_string() })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if let ActiveModal::NewRemoteWizard { name, .. } = &mut this.active_modal {
                                                            *name = val.to_string();
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
                                    .child(div().text_size(px(13.0)).font_weight(FontWeight::SEMIBOLD).child("Choose Provider Type:"))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .max_h(px(240.0))
                                            .overflow_hidden()
                                            .children(popular_backends.into_iter().map(|(backend_id, title, desc)| {
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
                                    ),
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
                                        let n = cur_name.clone();
                                        let b = cur_backend.clone();

                                        cx.spawn(async move |this, cx| {
                                            // Start wizard via driver
                                            let state_arc = app.state();
                                            let s = state_arc.read().await;
                                            if let Some(ref rc) = s.rc_client {
                                                let mut driver = rcm_rc::WizardDriver::new(rc.clone(), &n, &b, std::collections::BTreeMap::new(), false);
                                                let step_res = driver.start().await;

                                                let _ = this.update(cx, |this, cx| {
                                                    match step_res {
                                                        Ok(rcm_rc::WizardStep::AskQuestion { state, option, error }) => {
                                                            this.active_modal = ActiveModal::NewRemoteWizard {
                                                                step: 1,
                                                                name: n,
                                                                selected_backend: b,
                                                                filter: String::new(),
                                                                question_text: state,
                                                                question_help: option.help,
                                                                answer_input: String::new(),
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
                                                                filter: String::new(),
                                                                question_text: "OAuth Authorization".to_string(),
                                                                question_help: "Please complete authentication in your browser.".to_string(),
                                                                answer_input: String::new(),
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
                                Button::new("wizard_done_btn")
                                    .label("Finish Setup ✓")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_modal(cx);
                                        this.refresh_state(cx);
                                    }))
                            }),
                    ),
            )
    }
}
