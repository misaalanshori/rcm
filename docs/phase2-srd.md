# Rclone Manager (RCM) — Phase 2 Software Requirements Specification (SRD)

| | |
|---|---|
| **Document** | Software Requirements Specification (Phase 2), v1.0 |
| **Parent Document** | Combined SRS + SDD (`docs/rman-srdd.md`) |
| **Companion Document** | Detailed User Flows & Interaction Guide (`docs/user-flows.md`) |
| **Methodology** | **EARS (Easy Approach to Requirements Syntax)** |
| **Scope** | End-to-end user interactions, automated lifecycle, GUI components, and non-functional constraints |

---

## 1. Specification Framework: EARS Syntax Rules

Every requirement in this document is authored strictly using the **Easy Approach to Requirements Syntax (EARS)**:

- **Ubiquitous (Universal):** `The <system> shall <system response>.`
- **Event-Driven:** `When <trigger>, the <system> shall <system response>.`
- **State-Driven:** `While <pre-condition / state>, the <system> shall <system response>.`
- **Unwanted Behavior (Error / Fault Condition):** `If <trigger / error condition>, then the <system> shall <system response>.`
- **Optional Feature:** `Where <feature-specific condition>, the <system> shall <system response>.`
- **Complex Combinations:** `While <state>, when <trigger>, the <system> shall <system response>.` or `While <state>, if <error condition>, then the <system> shall <system response>.`

---

## 2. System Identifiers & Prefixes

- **`FR-LC`**: Daemon & Application Lifecycle Requirements
- **`FR-RM`**: Remote Configuration & Wizard Requirements
- **`FR-MT`**: Mounts & Virtual Drive Requirements
- **`FR-SV`**: Network Serves Requirements
- **`FR-FL`**: File Explorer & Data Transfers Requirements
- **`FR-BK`**: Backups, Diffs & Config Integrity Requirements
- **`FR-UI`**: Navigation, Command Palette & UI Component Requirements
- **`NFR-PF`**: Performance & Footprint Requirements
- **`NFR-SC`**: Security & Secret Protection Requirements
- **`NFR-RL`**: Reliability & Fault Tolerance Requirements
- **`NFR-AC`**: Accessibility & UX Requirements

---

## 3. Functional Requirements (EARS)

### 3.1 First-Time Launch & Lifecycle Management (`FR-LC`)

- **FR-LC-01 [Ubiquitous]:**  
  The system shall support zero-configuration launch through the standalone `rcm.exe` desktop executable without requiring prior terminal execution.
- **FR-LC-02 [Event-Driven]:**  
  When `rcm.exe` is executed, the UI process shall attempt to connect to `rcm-agent` over the local IPC channel (`\\.\pipe\rcm-<username>` on Windows or `$XDG_RUNTIME_DIR/rcm/ipc.sock` on Linux).
- **FR-LC-03 [Unwanted Behavior]:**  
  If connection to `rcm-agent` fails on launch, then `rcm.exe` shall automatically spawn `rcm-agent.exe --background` using Windows `CREATE_NO_WINDOW` and retry connection until established or a 3.5-second timeout elapses.
- **FR-LC-04 [State-Driven]:**  
  While `rcm-agent` is initializing, if no verified `rclone` binary exists in the managed directory (`%LOCALAPPDATA%\RCM\rclone\`), then `rcm-agent` shall automatically query `https://downloads.rclone.org/version.txt`, download the matching official zip archive, verify the security version floor ($\ge$ v1.73.5), extract the binary, and set it as `current` in `binary.json`.
- **FR-LC-05 [Event-Driven]:**  
  When `rcm-agent` establishes communication with the UI, the agent shall query the daemon status (`daemon.status`) and automatically execute `daemon.start` if the daemon is currently `Stopped`.
- **FR-LC-06 [Ubiquitous]:**  
  The agent shall spawn `rclone rcd` with `--use-json-log`, `--log-level NOTICE`, `--rc-job-expire-duration 1h`, per-session randomized credentials passed strictly via environment variables (`RCLONE_RC_USER`, `RCLONE_RC_PASS`), and bound exclusively to loopback (`127.0.0.1:<port>`).
- **FR-LC-07 [State-Driven]:**  
  While `rclone rcd` is running, the agent shall execute health check probes (`rc/noop`) every 5 seconds.
- **FR-LC-08 [Unwanted Behavior]:**  
  If `rclone rcd` crashes, then `rcm-agent` shall restart it with exponential backoff (1s doubling to a 60s cap).
- **FR-LC-09 [Unwanted Behavior]:**  
  If `rclone rcd` crashes 5 times within a 120-second rolling window, then `rcm-agent` shall trip the crash-loop breaker, transition to `Failed` state, record the last error lines from the log buffer, and cease restart attempts until user intervention.
- **FR-LC-10 [Event-Driven]:**  
  When the user closes the `rcm.exe` window, the UI process shall terminate immediately and release all GPU rendering resources, while `rcm-agent` and all running mounts shall remain active in the background (`DM-7`).
- **FR-LC-11 [Optional Feature]:**  
  Where the user enables "Autostart at Login" in Settings, the agent shall create an autostart entry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` on Windows or XDG desktop entry on Linux) that starts `rcm-agent.exe --background` silently at user logon (`DM-6`).
- **FR-LC-12 [Event-Driven]:**  
  When `rcm-agent` starts at logon, the reconciler shall automatically restore all enabled mount and serve profiles marked with `autostart = true`.

---

### 3.2 Cloud Remote Configuration & Interactive Wizard (`FR-RM`)

- **FR-RM-01 [Event-Driven]:**  
  When the user clicks `+ New Remote Wizard` on either the Dashboard or the Remotes view, the UI shall display the modal overlay titled `"New Remote Wizard — Select Cloud Backend (Step 1 of 2)"`.
- **FR-RM-02 [Ubiquitous]:**  
  The wizard backend selector shall render available cloud providers dynamically from `config/providers` (including Google Drive, Amazon S3, Dropbox, OneDrive, WebDAV, SFTP, and Crypt) with title, prefix, and descriptions.
- **FR-RM-03 [Event-Driven]:**  
  When the user types text into the provider search filter, the UI shall filter the backend list in real time matching provider names, prefixes, or descriptions.
- **FR-RM-04 [Event-Driven]:**  
  When the user inputs a remote name and clicks `Continue to Configuration →`, the UI shall validate the name format and reject any name containing empty characters, `:`, `/`, or `\`.
- **FR-RM-05 [Event-Driven]:**  
  When the initial backend and name are validated, the UI shall initialize `WizardDriver` and execute `config/create` with `nonInteractive=true` on the daemon.
- **FR-RM-06 [State-Driven]:**  
  While the wizard state machine receives `WizardStep::AskQuestion`, the UI shall dynamically render the question name, reflowed help text, and the corresponding input field derived from `OptionSchema` (masked password field if `is_password=true`, dropdown selection if `exclusive=true`, toggle switch if `type="bool"`).
- **FR-RM-07 [Event-Driven]:**  
  When the user submits an answer to a wizard question, the UI shall forward the answer to `config/update` with `continue=true` and echo the question state.
- **FR-RM-08 [Event-Driven]:**  
  When rclone requires OAuth authorization (`WizardStep::OAuthInProgress`), the UI shall display the OAuth URL and provide a prominent button labeled `🌐 Open Auth in Browser`.
- **FR-RM-09 [Event-Driven]:**  
  When the user clicks `🌐 Open Auth in Browser`, the system shall invoke the default operating system web browser pointing to the authorization URL via `rcm_platform::opener::open_url_in_browser`.
- **FR-RM-10 [State-Driven]:**  
  While waiting for OAuth authorization, the UI shall periodically poll `config/oauthstatus` until tokens are captured or the user clicks `Cancel`.
- **FR-RM-11 [Event-Driven]:**  
  When the user clicks `Cancel` during OAuth or wizard execution, the UI shall execute `config/oauthstop`, delete the incomplete remote section, and return to the Remotes view.
- **FR-RM-12 [Event-Driven]:**  
  When the wizard receives `WizardStep::Completed`, the system shall execute `fscache/clear`, record a `pre-mutation` snapshot of `rclone.conf`, dismiss the modal, refresh the remote list, and display the newly created remote with a `cloud` badge.
- **FR-RM-13 [Event-Driven]:**  
  When the user clicks `🗑 Delete` on any remote card, the system shall compute the remote dependency graph from `config/dump` (`CF-6`) and display a confirmation prompt warning if dependent remotes (e.g. Crypt or Union) point to that remote.
- **FR-RM-14 [Event-Driven]:**  
  When the user confirms remote deletion, the agent shall execute `config/delete` via the serialized mutation queue, clear `fscache`, and refresh the view.

---

### 3.3 Virtual Drive Mounts & Presets (`FR-MT`)

- **FR-MT-01 [Event-Driven]:**  
  When the user clicks `💽 + New Mount` or `+ New Mount Profile`, the UI shall open the modal dialog titled `"Create New Mount Profile"`.
- **FR-MT-02 [Ubiquitous]:**  
  The mount modal shall allow the user to select from all currently configured cloud remotes, select a target drive letter (`G:`, `M:`, `Z:`, `X:`, `Y:`, `P:` or Auto `*`), and select a mount preset.
- **FR-MT-03 [Ubiquitous]:**  
  The system shall provide five standardized mount presets mapping to concrete rclone flags:
  - **Balanced:** `vfs_cache_mode=writes`, `dir_cache_time=30m`
  - **Streaming:** `vfs_cache_mode=full`, `vfs_read_ahead=256M`, `vfs_cache_max_age=6h`
  - **Offline-First:** `vfs_cache_mode=full`, `vfs_cache_max_size=50G`, `vfs_cache_max_age=720h`
  - **Max Compatibility:** `vfs_cache_mode=full`, `no_checksum=true`, `no_modtime=true`, Windows network mode enabled
  - **Read-Only:** `read_only=true`, `dir_cache_time=1h`
- **FR-MT-04 [Unwanted Behavior]:**  
  If Windows network mode is enabled and the target is configured as a local folder mountpoint (rather than a drive letter), then the UI validation engine shall reject the profile and display an actionable explanation (`MT-4`).
- **FR-MT-05 [Event-Driven]:**  
  When the user clicks `Save & Mount as X:`, the UI shall serialize the profile into `%APPDATA%\RCM\profiles.toml` via IPC (`profiles.save_mount`), dispatch `mount/mount` to `rcd`, update the profile status to `● Mounted`, and close the modal.
- **FR-MT-06 [Event-Driven]:**  
  When the user clicks `📂 Open` on any mounted drive card, the system shall launch Windows File Explorer targeted at the drive root path (e.g. `explorer.exe G:\`) via `rcm_platform::opener::open_path_in_file_manager`.
- **FR-MT-07 [Event-Driven]:**  
  When the user clicks `⏹ Unmount` on an active mount card, the system shall execute `mount/unmount` with `mountPoint="<drive>:"` on `rcd` and update the status badge to `○ Stopped`.
- **FR-MT-08 [Event-Driven]:**  
  When the user clicks `▶ Mount` on a stopped mount card, the system shall execute `mount/mount` using the stored profile options and transition the card to `● Mounted`.
- **FR-MT-09 [Event-Driven]:**  
  When the user clicks `🗑 Delete` on a stopped mount card, the system shall delete the profile from `profiles.toml` via IPC (`profiles.delete_mount`) and remove the card from the UI.
- **FR-MT-10 [State-Driven]:**  
  While inspecting running mounts via `mount/listmounts`, if a mount exists that is not defined in `profiles.toml`, then the reconciler shall flag the mount as an unmanaged mount with an adoption option (`MT-7`).

---

### 3.4 Network Serves (WebDAV, SFTP, HTTP, S3) (`FR-SV`)

- **FR-SV-01 [Event-Driven]:**  
  When the user navigates to `[4] Serves`, the UI shall list all configured and active network serve profiles.
- **FR-SV-02 [Event-Driven]:**  
  When the user creates a serve profile, the system shall support protocols discovered at runtime from `serve/types` (including WebDAV, SFTP, HTTP, and S3).
- **FR-SV-03 [Unwanted Behavior]:**  
  If the user binds a serve profile to a non-loopback address (`0.0.0.0` or external LAN IP) without specifying username and password credentials, then the validation engine shall reject the profile per requirement `SV-3`.
- **FR-SV-04 [Event-Driven]:**  
  When the user starts a serve profile, the system shall execute `serve/start` on `rcd`, store the returned server `id`, and display a copyable endpoint URL badge (e.g. `http://192.168.1.50:8080`).
- **FR-SV-05 [Event-Driven]:**  
  When the user clicks `Stop` on an active serve card, the system shall execute `serve/stop` passing the server `id`.

---

### 3.5 File Explorer & Remote Browsing (`FR-FL`)

- **FR-FL-01 [Event-Driven]:**  
  When the user navigates to `[5] Files`, the UI shall present a remote selector displaying all configured cloud remotes.
- **FR-FL-02 [Event-Driven]:**  
  When a remote is selected, the system shall execute `operations/list` on `rcd` for that remote root and render the directory contents in a virtualized table.
- **FR-FL-03 [Ubiquitous]:**  
  The file table shall display each item's icon (`📁` for directory, `📄` for file), name, human-readable size (KB/MB/GB or `DIR`), and last-modified timestamp.
- **FR-FL-04 [Event-Driven]:**  
  When the user clicks a directory row, the view shall navigate into the subfolder and update the breadcrumb path.

---

### 3.6 Automated Backups, Redacted Diffs & Safe Config Restore (`FR-BK`)

- **FR-BK-01 [Event-Driven]:**  
  When any config mutation intent (`CreateRemote`, `UpdateRemote`, `DeleteRemote`, `RenameRemote`) is submitted to the agent's mutation queue, the system shall verify whether `rclone.conf` content has changed and save a deduplicated snapshot to `%LOCALAPPDATA%\RCM\backups\` labeled `rclone.conf.<timestamp>.pre-mutation.<hash8>` (`BK-1`).
- **FR-BK-02 [State-Driven]:**  
  While external changes to `rclone.conf` are detected by `ConfigWatcher`, the system shall record an `external-change` snapshot rate-limited to at most one snapshot per 10 minutes (`CF-9`).
- **FR-BK-03 [Event-Driven]:**  
  When the user views a snapshot diff in Settings, the diff renderer shall automatically replace all sensitive keys (passwords, tokens, secret keys) with `***REDACTED***` (`CI-5`).
- **FR-BK-04 [Event-Driven]:**  
  When the user requests a whole-file restore of a snapshot, the system shall execute the atomic Stop $\rightarrow$ Swap $\rightarrow$ Start sequence (`BK-2`, `§7.5`):
  1. Record a `pre-restore` snapshot of the current configuration.
  2. Unmount all active drives (`mount/unmountall`) and stop `rcd` (`core/quit`).
  3. Replace `rclone.conf` atomically with the snapshot file using replace-existing write-through semantics.
  4. Restart `rcd` and verify readability (`config/listremotes`).
  5. Resume reconciliation of active mounts and serves.
- **FR-BK-05 [Unwanted Behavior]:**  
  If the restore swap sequence fails verification on restart, then the system shall swap back to the `pre-restore` snapshot and surface the error to the user.

---

### 3.7 UI Navigation, Command Palette & Keyboard Completeness (`FR-UI`)

- **FR-UI-01 [Ubiquitous]:**  
  The native window shall feature a top navigation bar providing immediate access to the six primary destinations:
  - `[1] Dashboard`
  - `[2] Remotes`
  - `[3] Mounts`
  - `[4] Serves`
  - `[5] Files`
  - `[6] Settings`
- **FR-UI-02 [Event-Driven]:**  
  When the user presses keys `1` through `6` on the keyboard, the UI shall instantly switch the active destination view.
- **FR-UI-03 [Event-Driven]:**  
  When the user presses `Ctrl+K`, the UI shall open the global Command Palette search overlay.
- **FR-UI-04 [Event-Driven]:**  
  When the user types text into the Command Palette, the palette shall filter matching views and actions in real time.
- **FR-UI-05 [Event-Driven]:**  
  When the user presses `Esc` while any modal or the Command Palette is visible, the UI shall dismiss the overlay.
- **FR-UI-06 [Event-Driven]:**  
  When the user presses `q` while no text input is focused, the UI shall exit the window cleanly.

---

## 4. Non-Functional Requirements (EARS)

### 4.1 Performance & Footprint (`NFR-PF`)

- **NFR-PF-01 [Ubiquitous]:**  
  The total on-disk size of RCM's compiled binaries (`rcm.exe`, `rcm-agent.exe`, `rcmctl.exe`, `xtask.exe`) combined shall not exceed 40 MB (`NF-13`).
- **NFR-PF-02 [State-Driven]:**  
  While idle in the background, `rcm-agent.exe` shall consume $\le$ 30 MB RSS memory without initializing GPU libraries (`NF-1`).
- **NFR-PF-03 [State-Driven]:**  
  While the native GPUI window is open, `rcm.exe` shall consume $\le$ 150 MB memory (`NF-11`).
- **NFR-PF-04 [Event-Driven]:**  
  When `rcm.exe` is launched, cold start to interactive window rendering shall complete in $\le$ 1.5 seconds on mid-range hardware (`NF-2`).
- **NFR-PF-05 [Ubiquitous]:**  
  All asynchronous I/O (Named Pipes IPC, hyper RC client, process monitoring) shall execute on a dedicated multi-threaded Tokio runtime to prevent thread-local reactor switching or UI thread blocking (`§7.11`).

---

### 4.2 Security & Protection of Secrets (`NFR-SC`)

- **NFR-SC-01 [Ubiquitous]:**  
  `rcd` authentication credentials shall be generated per-session using cryptographically secure random tokens and passed strictly via environment variables, never on the command line argv (`R7`, `§9`).
- **NFR-SC-02 [Ubiquitous]:**  
  The IPC channel shall be restricted exclusively to the current user's security context (Windows Named Pipe with user-only DACL; Unix socket 0600 in `$XDG_RUNTIME_DIR`).
- **NFR-SC-03 [Ubiquitous]:**  
  All sensitive credentials (passwords, tokens, secret keys) in UI buffers, diff displays, logs, and diagnostics bundles shall be masked by default with explicit reveal actions (`UX-3`, `CI-5`).
- **NFR-SC-04 [Ubiquitous]:**  
  RCM binaries shall contain no external telemetry or network client stack; all remote cloud interactions and update downloads shall be executed exclusively by `rclone` or native platform tools (`NF-9`, `NF-16`).

---

### 4.3 Reliability & Config Durability (`NFR-RL`)

- **NFR-RL-01 [Ubiquitous]:**  
  `rcd` shall be the sole writer of `rclone.conf` during normal operation, ensuring that automatic OAuth token refreshes and UI edits never suffer race conditions or lost updates (`R12`, `CI-1`).
- **NFR-RL-02 [State-Driven]:**  
  While a configuration restore or mutation is in flight, the agent's mutation queue shall reject concurrent mutation intents to preserve atomicity (`CI-2`, `CI-4`).
- **NFR-RL-03 [Unwanted Behavior]:**  
  If a process crash or sudden termination occurs during a configuration mutation, the system shall guarantee that `rclone.conf` is always in an old-or-new valid state, never partially written or corrupt (`NF-4`).

---

### 4.4 Accessibility & UX Principles (`NFR-AC`)

- **NFR-AC-01 [Ubiquitous]:**  
  The UI shall adhere to progressive disclosure (`UX-6`): primary workflows (presets, essential inputs, one-click mount) shall be visible by default, with advanced flags disclosed on demand.
- **NFR-AC-02 [Ubiquitous]:**  
  Every interactive control shall expose AccessKit accessibility names, roles, and states for screen-reader compatibility (`NF-8`).
- **NFR-AC-03 [Ubiquitous]:**  
  The application shall support full keyboard navigation without requiring mouse interaction across all primary workflows (`NF-8`).

---

## 5. Requirement Traceability Matrix

| Requirement ID | EARS Pattern | User Flow (`user-flows.md`) | SRDD ID (`rman-srdd.md`) | Test Case / Suite |
|---|---|---|---|---|
| `FR-LC-01` | Ubiquitous | Flow 1 (First Launch) | `DM-7`, `§7.15` | `bootstrap_tests::*` |
| `FR-LC-02` | Event-Driven | Flow 1 (First Launch) | `DM-10`, `§7.14` | `ipc_tests::*` |
| `FR-LC-03` | Unwanted Behavior | Flow 1 (First Launch) | `DM-2`, `§7.1` | `bootstrap_tests::*` |
| `FR-LC-04` | State-Driven | Flow 1 (First Launch) | `BU-1`, `BU-4` | `supervisor_tests::test_bu_1_*` |
| `FR-LC-06` | Ubiquitous | Flow 1 (First Launch) | `DM-2`, `§9` | `supervisor_tests::*` |
| `FR-LC-08` | Unwanted Behavior | Flow 1 (First Launch) | `DM-4` | `supervisor_tests::test_dm_4_*` |
| `FR-LC-09` | Unwanted Behavior | Flow 1 (First Launch) | `DM-4` | `supervisor_tests::test_dm_4_*` |
| `FR-LC-10` | Event-Driven | Flow 9 (Window Close) | `DM-7` | `ui_state_tests::*` |
| `FR-RM-01` | Event-Driven | Flow 2 (Remote Wizard) | `CF-2` | `wizard_ui_tests::*` |
| `FR-RM-05` | Event-Driven | Flow 2 (Remote Wizard) | `R2`, `§7.4` | `client_tests::test_r2_*` |
| `FR-RM-06` | State-Driven | Flow 2 (Remote Wizard) | `R3`, `§7.3` | `form_engine_tests::*` |
| `FR-RM-08` | Event-Driven | Flow 2 (Remote Wizard) | `CF-3` | `client_tests::test_cf_3_*` |
| `FR-RM-12` | Event-Driven | Flow 2 (Remote Wizard) | `CI-3`, `BK-1` | `config_tests::test_bk_1_*` |
| `FR-RM-13` | Event-Driven | Flow 2 (Remote Wizard) | `CF-6` | `domain_tests::test_cf_6_*` |
| `FR-MT-01` | Event-Driven | Flow 3 (Mount Creation) | `MT-1` | `wizard_ui_tests::*` |
| `FR-MT-03` | Ubiquitous | Flow 3 (Mount Creation) | `MT-2` | `domain_tests::test_mt_1_mt_2_*` |
| `FR-MT-04` | Unwanted Behavior | Flow 3 (Mount Creation) | `MT-4` | `domain_tests::test_mt_4_*` |
| `FR-MT-05` | Event-Driven | Flow 3 (Mount Creation) | `MT-1`, `MT-5` | `supervisor_tests::test_reconciler_*` |
| `FR-MT-06` | Event-Driven | Flow 3 (Mount Creation) | `MT-5` | `platform_tests::*` |
| `FR-MT-07` | Event-Driven | Flow 4 (Unmounting) | `MT-5` | `supervisor_tests::*` |
| `FR-SV-03` | Unwanted Behavior | Flow 6 (Serves) | `SV-3` | `domain_tests::test_sv_3_*` |
| `FR-FL-02` | Event-Driven | Flow 5 (Files) | `OT-1` | `client_tests::*` |
| `FR-BK-01` | Event-Driven | Flow 7 (Backups) | `BK-1` | `config_tests::test_bk_1_*` |
| `FR-BK-03` | Event-Driven | Flow 7 (Backups) | `CI-5`, `BK-2` | `config_tests::test_ci_5_bk_2_*` |
| `FR-BK-04` | Event-Driven | Flow 7 (Backups) | `BK-2`, `§7.5` | `config_tests::test_bk_2_*` |
| `FR-UI-01` | Ubiquitous | Flow 8 (Navigation) | `UX-6`, `§7.11` | `tui_tests::*` |
| `FR-UI-03` | Event-Driven | Flow 8 (Command Palette) | `UX-2` | `ui_state_tests::*` |
| `NFR-PF-01` | Ubiquitous | Footprint | `NF-13` | `xtask_tests::*` |
| `NFR-SC-01` | Ubiquitous | Threat Model | `R7`, `§9` | `supervisor_tests::*` |
| `NFR-RL-01` | Ubiquitous | Config Integrity | `CI-1`, `R12` | `config_tests::*` |
