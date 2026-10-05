use gpui_kit::component::badge::Badge;
use gpui_kit::component::button::Button;
use gpui_kit::*;
use rcm_core::DaemonState;
use rcm_ui_kit::view_model::{MountItemViewModel, RemoteItemViewModel, ServeItemViewModel, SidebarDestination};
use crate::app::AppController;

pub struct RcmDesktopWindow {
    app: AppController,
    current_destination: SidebarDestination,
    daemon_state: DaemonState,
    status_text: String,
    remotes: Vec<RemoteItemViewModel>,
    mounts: Vec<MountItemViewModel>,
    serves: Vec<ServeItemViewModel>,
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
        }
    }

    pub fn set_destination(&mut self, dest: SidebarDestination, cx: &mut Context<Self>) {
        self.current_destination = dest;
        cx.notify();
    }

    pub fn refresh_state(&mut self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        cx.spawn(async move |this, cx| {
            let (daemon_state, remotes, mounts, serves) = app.fetch_ui_data().await;
            let _ = this.update(cx, |this, cx| {
                this.daemon_state = daemon_state;
                this.remotes = remotes;
                this.mounts = mounts;
                this.serves = serves;
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
                    .child(div().child("Rclone Manager v0.1.0 (Native GPUI)")),
            )
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
                "Click Refresh to start".to_string(),
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
                    .child(self.stat_card("Active Mounts", &self.mounts.len().to_string(), "Virtual drives mounted", rgb(0x89b4fa)))
                    .child(self.stat_card("Active Serves", &self.serves.len().to_string(), "WebDAV / SFTP endpoints", rgb(0xa6e3a1)))
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
                                    .label("⚡ Reconcile Mounts")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.trigger_reconcile(cx);
                                    })),
                            )
                            .child(
                                Button::new("nav_remotes_btn")
                                    .label("☁ Manage Remotes")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_destination(SidebarDestination::Remotes, cx);
                                    })),
                            )
                            .child(
                                Button::new("nav_serves_btn")
                                    .label("🌐 Network Serves")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_destination(SidebarDestination::Serves, cx);
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

    fn render_remotes_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(Button::new("add_remote_btn").label("+ New Remote Wizard")),
            )
            .child(
                if self.remotes.is_empty() {
                    div()
                        .p_8()
                        .rounded_lg()
                        .bg(rgb(0x1e1e2e))
                        .text_color(rgb(0xa6adc8))
                        .child("No remotes configured yet. Click '+ New Remote Wizard' or add one via rclone CLI.")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(self.remotes.iter().map(|r| {
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
                                        .child(div().font_weight(FontWeight::SEMIBOLD).child(r.name.clone()))
                                        .child(Badge::new().child("cloud")),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_2()
                                        .child(Button::new(format!("del_{}", r.name)).label("Delete")),
                                )
                        }))
                },
            )
    }

    fn render_mounts_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(Button::new("add_mount_btn").label("+ New Mount Profile")),
            )
            .child(
                if self.mounts.is_empty() {
                    div()
                        .p_8()
                        .rounded_lg()
                        .bg(rgb(0x1e1e2e))
                        .text_color(rgb(0xa6adc8))
                        .child("No mount profiles found. Create a profile to mount a remote as drive letter G:, M:, etc.")
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(self.mounts.iter().map(|m| {
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
                                        .child(div().font_weight(FontWeight::BOLD).text_color(rgb(0x89b4fa)).child(m.target.clone()))
                                        .child(div().child(format!("{} ({})", m.name, m.remote)))
                                        .child(Badge::new().child(m.preset.clone())),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_2()
                                        .child(Button::new(format!("mnt_{}", m.name)).label("Mount")),
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
                        .child("No active serves. Create a serve profile to share cloud files over LAN or internet.")
                } else {
                    div().child("Serves configured.")
                },
            )
    }

    fn render_files_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(22.0)).font_weight(FontWeight::BOLD).child("File Explorer"))
                    .child(div().text_size(px(13.0)).text_color(rgb(0xa6adc8)).child("Browse and inspect files directly across your remotes.")),
            )
            .child(
                div()
                    .p_8()
                    .rounded_lg()
                    .bg(rgb(0x1e1e2e))
                    .text_color(rgb(0xa6adc8))
                    .child("Select a remote from the top dropdown to browse directory contents."),
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
                            ),
                    ),
            )
    }
}
