# Rclone Manager (RCM) — Phase 2 Detailed Technical Specification (SRD)

| | |
|---|---|
| **Document** | Software Requirements Specification (Phase 2 Detailed Technical Spec), v2.0 |
| **Parent Document** | Combined SRS + SDD (`docs/rman-srdd.md`) |
| **Companion Document** | Detailed User Flows & Interaction Guide (`docs/user-flows.md`) |
| **Specification Standard** | **EARS (Easy Approach to Requirements Syntax)** |
| **Scope** | Exhaustive technical requirements covering every user interaction, background lifecycle event, RC/IPC protocol transaction, GUI component, error recovery path, and non-functional constraint |

---

## 1. Specification Framework: EARS Syntax Rules

Every requirement in this document is authored strictly using the **Easy Approach to Requirements Syntax (EARS)**:

- **Ubiquitous (Universal):**  
  `The <system> shall <system response>.`
- **Event-Driven:**  
  `When <trigger>, the <system> shall <system response>.`
- **State-Driven:**  
  `While <pre-condition / state>, the <system> shall <system response>.`
- **Unwanted Behavior (Error / Fault Condition):**  
  `If <trigger / error condition>, then the <system> shall <system response>.`
- **Optional Feature:**  
  `Where <feature-specific condition>, the <system> shall <system response>.`
- **Complex Combinations:**  
  `While <state>, when <trigger>, the <system> shall <system response>.`  
  `While <state>, if <error condition>, then the <system> shall <system response>.`

### System Boundaries and Actors
- **`UI` / `rcm`:** The native hardware-accelerated desktop window process (built with GPUI/GPUI Kit).
- **`Agent` / `rcm-agent`:** The resident, headless background supervisor and reconciler daemon.
- **`Helper CLI` / `rcmctl`:** The multi-call command-line interface for scripting and `--password-command`.
- **`Daemon` / `rcd`:** The official bundled `rclone rcd` background process controlled via HTTP JSON-RPC over loopback.
- **`WinFsp / FUSE`:** The operating system kernel-mode filesystem drivers enabling native drive letter and directory mounts.
- **`IPC Channel`:** Authenticated local inter-process communication (Windows Named Pipes / Linux Unix domain sockets).

---

## 2. Requirement Numbering & Classification

- **`FR-LC-xxx`**: Lifecycle, Auto-Provisioning & Supervisor Requirements
- **`FR-RM-xxx`**: Cloud Remote Management & Interactive Wizard Requirements
- **`FR-MT-xxx`**: Virtual Drive Mounts, Presets & WinFsp Requirements
- **`FR-SV-xxx`**: Network Storage Services (WebDAV, SFTP, HTTP, S3) Requirements
- **`FR-FL-xxx`**: Remote File Explorer & Transfer Jobs Requirements
- **`FR-BK-xxx`**: Backups, Redacted Diffs & Safe Config Restore Requirements
- **`FR-UI-xxx`**: UI Architecture, Navigation & Command Palette Requirements
- **`NFR-PF-xxx`**: Performance, Latency & Memory Footprint Requirements
- **`NFR-SC-xxx`**: Security, Cryptography & Secret Protection Requirements
- **`NFR-RL-xxx`**: Reliability, Durability & Crash Recovery Requirements
- **`NFR-TH-xxx`**: Concurrency, Runtime Isolation & Reactor Requirements
- **`NFR-AC-xxx`**: Accessibility, Localization & UX Principles

---

## 3. Functional Requirements (EARS)

### 3.1 First-Time Launch & Lifecycle Management (`FR-LC`)

#### Launch & Background Agent Handshake
- **FR-LC-01 [Ubiquitous]:**  
  The system shall support zero-configuration launch through the standalone `rcm.exe` desktop executable without requiring manual daemon startup or terminal execution.
- **FR-LC-02 [Event-Driven]:**  
  When `rcm.exe` starts, the UI shall probe the local IPC named pipe (`\\.\pipe\rcm-<username>` on Windows or `$XDG_RUNTIME_DIR/rcm/ipc.sock` on Linux) to determine whether `rcm-agent` is already resident.
- **FR-LC-03 [Unwanted Behavior]:**  
  If the IPC connection probe fails with a file-not-found or connection-refused error, then `rcm.exe` shall locate `rcm-agent.exe` (checking first in its own directory, then in `%LOCALAPPDATA%\Programs\RCM\`) and spawn `rcm-agent.exe --background --pipe <pipe_name>` using Windows `CREATE_NO_WINDOW` (flag `0x08000000`) so that no console window flashes.
- **FR-LC-04 [State-Driven]:**  
  While waiting for the spawned `rcm-agent` to initialize, `rcm.exe` shall poll the IPC pipe every 100 milliseconds for up to 35 attempts (3.5 seconds total).
- **FR-LC-05 [Unwanted Behavior]:**  
  If the IPC connection polling exceeds 3.5 seconds without a successful handshake, then `rcm.exe` shall display an error notification indicating that the background agent failed to initialize, while still rendering the UI shell in offline mode.

#### Automatic rclone Binary Provisioning
- **FR-LC-06 [State-Driven]:**  
  While `rcm-agent` initializes, the binary manager shall inspect `%LOCALAPPDATA%\RCM\rclone\` (Windows) or `~/.local/share/rcm/rclone/` (Linux) and `state/binary.json` to verify if a valid `rclone` executable exists.
- **FR-LC-07 [Unwanted Behavior]:**  
  If no valid `rclone` executable exists in the managed directory, then `rcm-agent` shall automatically query `https://downloads.rclone.org/version.txt` via native platform tools (`curl.exe` or `curl`), resolve the latest stable release tag (e.g. `v1.75.1`), and verify that the version satisfies the security version floor ($\ge$ v1.73.5 per SRDD BU-4).
- **FR-LC-08 [Event-Driven]:**  
  When the release version is verified, `rcm-agent` shall resolve the platform archive URL (`https://downloads.rclone.org/<version>/rclone-<version>-<os>-<arch>.zip`), download the archive to a temporary directory using `curl` with resume and redirect flags (`-f -L -o`), and extract the executable using `tar.exe -xf` (or PowerShell `Expand-Archive` fallback).
- **FR-LC-09 [Event-Driven]:**  
  When the executable is extracted, `rcm-agent` shall copy it into `%LOCALAPPDATA%\RCM\rclone\<version>\rclone.exe`, verify its execution by invoking `rclone version`, record the version in `state/binary.json` as `current`, and delete the temporary download archive.
- **FR-LC-10 [Unwanted Behavior]:**  
  If automatic binary download fails due to network unavailability, then `rcm-agent` shall log the network failure and report a clear error state over IPC when `daemon.start` is requested.

#### Daemon Supervision & Loopback Security
- **FR-LC-11 [Ubiquitous]:**  
  The agent shall allocate a free ephemeral loopback TCP port by temporarily binding `127.0.0.1:0` before spawning `rcd`.
- **FR-LC-12 [Ubiquitous]:**  
  The agent shall generate cryptographically secure randomized credentials for each daemon session (`RCLONE_RC_USER` and a 256-bit `RCLONE_RC_PASS`) and supply them strictly through process environment variables, never on the command line arguments (`R7`, `§9`).
- **FR-LC-13 [Event-Driven]:**  
  When spawning `rclone rcd`, the agent shall pass arguments: `rcd`, `--config`, `<path>`, `--rc-addr 127.0.0.1:<port>`, `--rc-job-expire-duration 1h`, `--use-json-log`, and `--log-level NOTICE`.
- **FR-LC-14 [Optional Feature]:**  
  Where the user's `rclone.conf` is password-encrypted, the agent shall append `--password-command "<rcmctl> print-config-pass"` to the spawn arguments, reading the master password securely from the operating system keyring without exposing secrets in process listings (`CF-8`, `§7.12`).
- **FR-LC-15 [Ubiquitous]:**  
  The agent shall pipe `rcd`'s standard output and standard error into a thread-safe, bounded in-memory ring buffer storing the most recent 5,000 log lines (`DM-9`, `§7.15`).
- **FR-LC-16 [State-Driven]:**  
  While `rcd` is starting, the agent shall poll `rc/noop` every 100 milliseconds for up to 50 attempts until the HTTP server responds with HTTP 200 OK.
- **FR-LC-17 [Event-Driven]:**  
  When `rc/noop` succeeds, the agent shall record `state/rcd.json` containing the process ID, start timestamp, executable path, config path, loopback address, execute ID, and session credentials (`DM-5`).
- **FR-LC-18 [Event-Driven]:**  
  When `rcm-agent` starts while an existing `rcd` is already running, the agent shall read `state/rcd.json`, verify that the PID, binary path, and config path match, test credentials with `rc/noop`, and adopt the running daemon instead of spawning a redundant process (`DM-5`).

#### Health Monitoring, Crash Backoff & Loop Breaker
- **FR-LC-19 [State-Driven]:**  
  While `rcd` is in `Ready` state, the agent shall execute health check probes (`rc/noop`) every 5 seconds.
- **FR-LC-20 [Event-Driven]:**  
  When a health check returns a new `executeId` different from the cached ID, the agent shall broadcast `RcdRestarted` to all connected UI clients so that stale job IDs are invalidated (`DM-3`, `R6`).
- **FR-LC-21 [Unwanted Behavior]:**  
  If `rcd` terminates unexpectedly (exit code non-zero or unexpected termination), then the supervisor shall transition the state to `Crashed`, record the timestamp, and calculate an exponential backoff delay using $\min(2^{\text{crashes}}, 60)$ seconds (`DM-4`).
- **FR-LC-22 [Unwanted Behavior]:**  
  If `rcd` records 5 crashes within any rolling 120-second window, then the supervisor shall trip the crash-loop breaker, transition to `Failed` state, halt automatic restart attempts, and surface the most recent error lines from the log ring buffer (`DM-4`).
- **FR-LC-23 [Event-Driven]:**  
  When `rcd` achieves a continuous healthy run of 60 seconds without crashing, the supervisor shall reset the crash count and clear the crash-loop breaker history.

#### Window Exit & Autostart Persistence
- **FR-LC-24 [Event-Driven]:**  
  When the user closes the `rcm.exe` window (clicking `[X]` or pressing `q`), the UI process shall exit immediately and release all GPU memory, while `rcm-agent` and all active mounts shall continue running in the background (`DM-7`).
- **FR-LC-25 [Optional Feature]:**  
  Where the user enables "Autostart at Login" in Settings, the agent shall write a registry entry to `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` with value `"\"<install_dir>\\rcm-agent.exe\" --background"` (Windows) or create an autostart desktop entry at `~/.config/autostart/rcm-agent.desktop` (Linux) (`DM-6`, `§7.10`).
- **FR-LC-26 [Event-Driven]:**  
  When the operating system logs in the user, `rcm-agent` shall launch in the background, start `rcd`, unlock encrypted configuration via the keyring helper if applicable, and automatically mount all profiles marked with `autostart = true`.

---

### 3.2 Cloud Remote Configuration & Interactive Wizard (`FR-RM`)

#### Wizard Trigger & Provider Selection
- **FR-RM-01 [Event-Driven]:**  
  When the user clicks `+ New Remote Wizard` or `☁ + Add Remote`, the UI shall open a modal dialog displaying Step 1 of the wizard.
- **FR-RM-02 [Ubiquitous]:**  
  The wizard shall query `config/providers` from the daemon and render available providers (including Google Drive, Amazon S3, Dropbox, Microsoft OneDrive, WebDAV, SFTP, and Crypt) with their display name, prefix identifier, and description.
- **FR-RM-03 [Event-Driven]:**  
  When the user types characters into the provider search box, the UI shall filter the backend list in real time using a case-insensitive substring match across names, prefixes, and descriptions.
- **FR-RM-04 [Event-Driven]:**  
  When the user inputs a remote name, the UI shall validate the input and reject any name that is empty, contains whitespace only, contains `:`, `/`, or `\`, or collides with a single-letter Windows drive letter (`A`..`Z`).
- **FR-RM-05 [Event-Driven]:**  
  When the user clicks `Continue to Configuration →` with a valid backend and name selected, the UI shall transition to Step 2 and call `config/create` with `nonInteractive=true` on `rcd`.

#### Dynamic Question Rendering & State Machine
- **FR-RM-06 [State-Driven]:**  
  While the wizard driver receives `WizardStep::AskQuestion { state, option, error }`, the UI shall dynamically render the question matching rclone's schema (`R2`, `CF-2`, `§7.3`):
  - If `option.is_password` or `option.sensitive` is true, the UI shall render a masked input field with a toggleable reveal eyeball button.
  - If `option.exclusive` is true and `option.examples` is non-empty, the UI shall render a selectable dropdown or segmented control listing the example values with their individual help tooltips.
  - If `option.exclusive` is false and `option.examples` is non-empty, the UI shall render a combobox with autocomplete suggestions.
  - If `option.option_type` is `"bool"`, the UI shall render a toggle switch defaulting to `option.default`.
  - If `option.option_type` is numeric or unit-bearing (`Duration`, `SizeSuffix`, `BandwidthSpec`), the UI shall render a validated numeric input with a unit selector badge.
  - If `option.advanced` is true, the UI shall collapse the question inside an expandable "Advanced Settings" accordion.
  - If `option.hide > 0`, the UI shall omit the question unless the user toggles "Show all questions" (`all=true`).
- **FR-RM-07 [Ubiquitous]:**  
  The UI shall split `option.help` at the first newline character, rendering the first line as an inline field description and the remainder inside an expandable popover.
- **FR-RM-08 [Event-Driven]:**  
  When the user submits an answer, the wizard driver shall execute `config/update` with `continue=true`, `state=<state>`, `result=<value>`, and include the default parameters on every continue call per rclone protocol requirements.
- **FR-RM-09 [Unwanted Behavior]:**  
  If rclone returns a validation error in `WizardStepResponse`, then the wizard driver shall re-render the current question with the verbatim error message highlighted in red above the input field.

#### OAuth Browser Authorization
- **FR-RM-10 [State-Driven]:**  
  While rclone executes an asynchronous OAuth exchange (`WizardStep::OAuthInProgress`), the UI shall display the authorization URL emitted by `config/oauthstatus` and provide a prominent button labeled `🌐 Open Auth in Browser` (`CF-3`).
- **FR-RM-11 [Event-Driven]:**  
  When the user clicks `🌐 Open Auth in Browser`, the system shall invoke the operating system's default browser to the consent URL using `rcm_platform::opener::open_url_in_browser`.
- **FR-RM-12 [State-Driven]:**  
  While the OAuth consent browser window is open, the UI shall poll `config/oauthstatus` every 500 milliseconds until the status changes to `ok` or `error`.
- **FR-RM-13 [Optional Feature]:**  
  Where the user is on a headless machine or remote session, the UI shall provide an alternative "Paste Token Manually" toggle, accepting tokens generated via `rclone authorize` on another machine.
- **FR-RM-14 [Event-Driven]:**  
  When the user clicks `Cancel` during OAuth or question answering, the wizard driver shall execute `config/oauthstop`, delete the incomplete remote section, and return to the Remotes view.

#### Completion, Snapshots & Dependency Integrity
- **FR-RM-15 [Event-Driven]:**  
  When the wizard receives `WizardStep::Completed`, the agent's mutation queue shall execute a pre-mutation snapshot of `rclone.conf` (`BK-1`), verify the newly created remote with `config/get`, execute `fscache/clear`, close the modal, and refresh the Remotes view (`CI-3`).
- **FR-RM-16 [Event-Driven]:**  
  When the user clicks `🗑 Delete` on any remote card, the system shall evaluate the remote dependency graph constructed from `config/dump` (`CF-6`) and identify all referencing remotes (Crypt wrappers, Union, Alias, Chunker).
- **FR-RM-17 [Unwanted Behavior]:**  
  If the remote targeted for deletion is referenced by other remotes, then the UI shall display a modal warning naming all referrers and requiring explicit user confirmation before proceeding.
- **FR-RM-18 [Event-Driven]:**  
  When remote deletion is confirmed, the agent shall execute `config/delete`, execute `fscache/clear`, and emit `ConfigChanged`.

---

### 3.3 Virtual Drive Mounts & Presets (`FR-MT`)

#### Mount Profile Creation & Target Validation
- **FR-MT-01 [Event-Driven]:**  
  When the user clicks `💽 + New Mount` or `+ New Mount Profile`, the UI shall open the modal dialog titled `"Create New Mount Profile"`.
- **FR-MT-02 [Ubiquitous]:**  
  The mount modal shall allow the user to select from all configured cloud remotes, specify an optional subpath (`remote:subpath`), select a target drive letter (`D:`..=`Z:` or Auto `*`), and choose a mount preset.
- **FR-MT-03 [Ubiquitous]:**  
  The system shall probe available Windows drive letters by querying unassigned letters and presenting available choices in the selector.
- **FR-MT-04 [Ubiquitous]:**  
  The system shall provide five standardized mount presets mapping to concrete rclone flags:
  - **Balanced:** `vfs_cache_mode=writes`, `dir_cache_time=30m`
  - **Streaming:** `vfs_cache_mode=full`, `vfs_read_ahead=256M`, `vfs_cache_max_age=6h`, `dir_cache_time=30m`
  - **Offline-First:** `vfs_cache_mode=full`, `vfs_cache_max_size=50G`, `vfs_cache_max_age=720h`, `dir_cache_time=24h`
  - **Max Compatibility:** `vfs_cache_mode=full`, `no_checksum=true`, `no_modtime=true`, Windows network mode enabled
  - **Read-Only:** `read_only=true`, `dir_cache_time=1h`
- **FR-MT-05 [Unwanted Behavior]:**  
  If Windows network mode is enabled and the target is configured as a local folder mountpoint (rather than a drive letter), then the validation engine shall reject the profile and explain that network mode requires a drive letter semantic (`MT-4`).
- **FR-MT-06 [State-Driven]:**  
  While rendering the mount modal on Windows, if WinFsp is not detected in the Windows registry (`HKLM\SOFTWARE\WinFsp`), then the UI shall display an amber notification warning that WinFsp is required to mount drives, offering a command to run `winget install WinFsp.WinFsp` or visit `https://winfsp.dev/` (`MT-4`, `IN-3`).

#### Mount Execution & Shell Integration
- **FR-MT-07 [Event-Driven]:**  
  When the user clicks `Save & Mount as X:`, the UI shall persist the profile to `%APPDATA%\RCM\profiles.toml` via IPC (`profiles.save_mount`), dispatch `mount/mount` with `fs`, `mountPoint`, `mountType="cmount"`, and the preset's flat VFS flags to `rcd`, update the status card to `● Mounted`, and dismiss the modal.
- **FR-MT-08 [Event-Driven]:**  
  When the user clicks `📂 Open` on any mounted drive card, the system shall invoke Windows File Explorer targeted at the drive root path (e.g. `explorer.exe G:\`) via `rcm_platform::opener::open_path_in_file_manager`.
- **FR-MT-09 [Event-Driven]:**  
  When the user clicks `⏹ Unmount` on an active mount card, the system shall execute `mount/unmount` with `mountPoint="<drive>:"` on `rcd` and update the status badge to `○ Stopped`.
- **FR-MT-10 [Unwanted Behavior]:**  
  If unmounting fails because files are currently open or locked by applications, then the system shall display an actionable prompt offering to force-unmount (`mount/unmount` with force flag).
- **FR-MT-11 [Event-Driven]:**  
  When the user clicks `▶ Mount` on any stopped mount card, the system shall execute `mount/mount` using the stored profile options and transition the card to `● Mounted`.
- **FR-MT-12 [Event-Driven]:**  
  When the user clicks `🗑 Delete` on any mount card, the system shall prompt for confirmation, delete the profile from `profiles.toml` via IPC (`profiles.delete_mount`), and unmount the drive if currently active.
- **FR-MT-13 [State-Driven]:**  
  While reconciling mounts, if a mount returned by `mount/listmounts` is not present in `profiles.toml`, then the reconciler shall flag the mount as an unmanaged mount and present an `Adopt` action (`MT-7`).

---

### 3.4 Network Storage Services (`FR-SV`)

- **FR-SV-01 [Event-Driven]:**  
  When the user navigates to `[4] Serves`, the UI shall list all configured and active network serve profiles.
- **FR-SV-02 [Ubiquitous]:**  
  The serve profile creation form shall support protocols dynamically discovered from `serve/types` (including WebDAV, SFTP, HTTP, S3, DLNA, and FTP).
- **FR-SV-03 [Ubiquitous]:**  
  The serve profile form shall default the bind address to loopback (`127.0.0.1:8080`).
- **FR-SV-04 [Unwanted Behavior]:**  
  If the user specifies a non-loopback bind address (`0.0.0.0` or a LAN IP) without providing both username and password credentials, then the validation engine shall reject the profile and enforce authentication per requirement `SV-3`.
- **FR-SV-05 [Event-Driven]:**  
  When the user starts a serve profile, the agent shall execute `serve/start` on `rcd`, record the returned server `id`, and display a copyable endpoint URL badge (e.g. `http://192.168.1.50:8080`).
- **FR-SV-06 [Event-Driven]:**  
  When the user clicks `Stop` on an active serve card, the agent shall execute `serve/stop` passing the server `id` and update the card status.

---

### 3.5 Remote File Explorer & Data Transfers (`FR-FL`)

- **FR-FL-01 [Event-Driven]:**  
  When the user navigates to `[5] Files`, the UI shall render a remote selector bar listing all configured cloud remotes.
- **FR-FL-02 [Event-Driven]:**  
  When a remote is selected, the UI shall execute `operations/list` on `rcd` for that remote root and display the directory contents in a virtualized table.
- **FR-FL-03 [Ubiquitous]:**  
  The file table shall display each entry's icon (`📁` for directory, `📄` for file), name, human-readable size (`DIR` for folders, or formatted B/KB/MB/GB), and modification timestamp.
- **FR-FL-04 [Event-Driven]:**  
  When the user clicks a directory row, the view shall navigate into that subdirectory, update the breadcrumb path bar, and query `operations/list` for the nested path.
- **FR-FL-05 [Event-Driven]:**  
  When the user clicks a segment in the breadcrumb path bar, the view shall navigate directly to the chosen parent directory.
- **FR-FL-06 [Event-Driven]:**  
  When the user clicks `Public Link` on any file, the UI shall execute `operations/publiclink` on `rcd` and display the generated public sharing link with a one-click copy button.
- **FR-FL-07 [Event-Driven]:**  
  When the user initiates a sync, copy, or move transfer job, the UI shall prompt for source and destination paths, provide a "Dry Run" preview toggle, and execute the job asynchronously with a unique `_group` identifier (`OT-2`).
- **FR-FL-08 [State-Driven]:**  
  While a transfer job is executing, the UI shall query `core/stats` for that job group every 1 second, displaying bytes transferred, transfer speed (MB/s), percentage complete, and an estimated time to completion (ETA).
- **FR-FL-09 [Event-Driven]:**  
  When the user clicks `Cancel Job`, the system shall execute `job/stop` on `rcd` for that job ID.

---

### 3.6 Automated Backups, Redacted Diffs & Safe Config Restore (`FR-BK`)

- **FR-BK-01 [Event-Driven]:**  
  When any config mutation intent (`CreateRemote`, `UpdateRemote`, `DeleteRemote`, `RenameRemote`) is submitted to the mutation queue, the agent shall compute the SHA-256 hash of `rclone.conf`.
- **FR-BK-02 [State-Driven]:**  
  While processing a mutation, if the computed hash differs from the newest stored snapshot, the agent shall save a snapshot file to `%LOCALAPPDATA%\RCM\backups\` named `rclone.conf.<timestamp>.pre-mutation.<hash8>` and update `index.json` (`BK-1`).
- **FR-BK-03 [State-Driven]:**  
  While external changes to `rclone.conf` are detected by `ConfigWatcher`, the agent shall take an `external-change` snapshot rate-limited to at most one snapshot per 10 minutes (`CF-9`).
- **FR-BK-04 [Ubiquitous]:**  
  The snapshot store shall enforce retention policy pruning (`SnapshotRetention`), retaining at most the last 50 pre-mutation snapshots, 30 daily snapshots, and 12 weekly snapshots (`BK-1`).
- **FR-BK-05 [Event-Driven]:**  
  When the user views a snapshot diff in Settings, the diff engine shall parse both configurations using the read-only INI parser and mask every key matching `pass`, `secret`, `token`, `key`, `auth`, or `credential` with `***REDACTED***` (`CI-5`).
- **FR-BK-06 [Event-Driven]:**  
  When the user executes a whole-file restore, the agent shall execute the atomic Stop $\rightarrow$ Swap $\rightarrow$ Start sequence (`BK-2`, `§7.5`):
  1. Record a `pre-restore` snapshot of the current configuration file.
  2. Pause the reconciler, call `mount/unmountall` and `serve/stopall`, and call `core/quit`.
  3. Wait for the `rcd` process to terminate (terminating with force if it does not exit within 10 seconds).
  4. Write the chosen snapshot to a temporary file in the config directory and execute an atomic rename over `rclone.conf` (`MoveFileExW` with replace-existing on Windows; `rename` with parent directory fsync on Linux).
  5. Start `rcd`, verify readability by calling `config/listremotes`, and unpause the reconciler.
- **FR-BK-07 [Unwanted Behavior]:**  
  If `rcd` fails to start or verify readability after the restore swap, then the agent shall immediately swap back to the `pre-restore` snapshot and notify the user with rclone's verbatim error message (`BK-4`).
- **FR-BK-08 [Optional Feature]:**  
  Where the user selects a single remote from a plaintext snapshot for selective restore, the agent shall execute `config/create` with `noObscure=true` and `nonInteractive=true` for that remote section, followed by `fscache/clear` (`BK-2`).

---

### 3.7 UI Navigation, Command Palette & Keyboard Completeness (`FR-UI`)

- **FR-UI-01 [Ubiquitous]:**  
  The native desktop window shall provide continuous navigation across the six primary destinations:
  - `[1] Dashboard`
  - `[2] Remotes`
  - `[3] Mounts`
  - `[4] Serves`
  - `[5] Files`
  - `[6] Settings`
- **FR-UI-02 [Event-Driven]:**  
  When the user presses number keys `1` through `6` on the keyboard, the UI shall instantly switch the active destination view.
- **FR-UI-03 [Event-Driven]:**  
  When the user presses `Ctrl+K`, the UI shall open the global Command Palette search overlay centered on screen.
- **FR-UI-04 [State-Driven]:**  
  While the Command Palette is open, typing characters into the query box shall filter views, actions, and mounts in real time.
- **FR-UI-05 [Event-Driven]:**  
  When the user presses `Up` or `Down` arrow keys in the Command Palette, the UI shall move the highlighted selection cursor across filtered items.
- **FR-UI-06 [Event-Driven]:**  
  When the user presses `Enter` on a selected Command Palette item, the UI shall execute the associated action or navigation and dismiss the palette.
- **FR-UI-07 [Event-Driven]:**  
  When the user presses `Esc` while any modal or the Command Palette is visible, the UI shall dismiss the overlay.
- **FR-UI-08 [Event-Driven]:**  
  When the user presses `q` while no text input field has focus, the UI shall close the application window.

---

## 4. Non-Functional Requirements (EARS)

### 4.1 Performance & Footprint (`NFR-PF`)

- **NFR-PF-01 [Ubiquitous]:**  
  The combined disk footprint of all RCM compiled binaries (`rcm.exe`, `rcm-agent.exe`, `rcmctl.exe`, `xtask.exe`) in a release build shall not exceed 40 MB (`NF-13`).
- **NFR-PF-02 [State-Driven]:**  
  While resident and idle in the background, `rcm-agent.exe` shall maintain a memory footprint $\le$ 30 MB RSS without initializing graphics or GPU drivers (`NF-1`).
- **NFR-PF-03 [State-Driven]:**  
  While the native GPUI desktop window is open and displaying a 10,000-row file list, `rcm.exe` shall maintain a memory footprint $\le$ 150 MB RSS (`NF-11`).
- **NFR-PF-04 [Event-Driven]:**  
  When `rcm.exe` is launched, cold start to interactive window rendering shall complete in $\le$ 1.5 seconds on mid-range hardware (`NF-2`).
- **NFR-PF-05 [State-Driven]:**  
  While scrolling through virtualized lists of up to 10,000 entries, the UI renderer shall maintain a minimum frame rate of 60 frames per second (`NF-3`).
- **NFR-PF-06 [State-Driven]:**  
  While the UI window is minimized or hidden, the UI shall cease all render passes and suspend RC polling timers (`NF-14`).

---

### 4.2 Concurrency, Runtime Isolation & Reactor Safety (`NFR-TH`)

- **NFR-TH-01 [Ubiquitous]:**  
  The system shall isolate all asynchronous Tokio I/O operations (Named Pipes IPC, hyper loopback RC client, and process monitors) onto a dedicated multi-threaded Tokio runtime managed by `AppController` (`§7.11`).
- **NFR-TH-02 [Ubiquitous]:**  
  Background async operations spawned from GPUI callbacks shall execute on the Tokio runtime via `tokio_handle.spawn(...)`, awaiting completion before crossing back into the GPUI context.
- **NFR-TH-03 [Unwanted Behavior]:**  
  No Tokio I/O future shall be polled directly by GPUI's `BackgroundExecutor` threads without an active Tokio runtime context, preventing reactor panics (`"there is no reactor running"`).

---

### 4.3 Security & Protection of Secrets (`NFR-SC`)

- **NFR-SC-01 [Ubiquitous]:**  
  The RC communication channel shall bind exclusively to loopback (`127.0.0.1`) on Windows or a Unix domain socket (`0600` permissions) on Linux, rejecting all external off-host connections (`R7`, `NF-6`).
- **NFR-SC-02 [Ubiquitous]:**  
  The IPC channel shall restrict permissions exclusively to the current logged-in user account (Windows Named Pipe with user-only DACL; Linux socket in `$XDG_RUNTIME_DIR` with `0600` permissions).
- **NFR-SC-03 [Ubiquitous]:**  
  All passwords, OAuth tokens, and secret keys stored in memory shall be held in zeroizing memory buffers (`Zeroizing<String>`) that overwrite memory upon deallocation (`§7.12`).
- **NFR-SC-04 [Ubiquitous]:**  
  All secrets displayed in UI text fields shall be masked by default and redacted from logs, diff outputs, and diagnostic bundles (`UX-3`, `CI-5`).
- **NFR-SC-05 [Ubiquitous]:**  
  The system shall contain no telemetry, usage tracking, or outbound analytics network calls (`NF-9`).
- **NFR-SC-06 [Ubiquitous]:**  
  RCM binaries shall contain no TLS or HTTP network client stack; all external downloads and cloud operations shall be executed strictly by `rclone` or native platform utilities (`NF-16`).

---

### 4.4 Reliability & Configuration Durability (`NFR-RL`)

- **NFR-RL-01 [Ubiquitous]:**  
  `rcd` shall be the sole writer of `rclone.conf` during normal operation, ensuring that runtime OAuth token refreshes and UI configuration edits never conflict or overwrite each other (`R12`, `CI-1`).
- **NFR-RL-02 [State-Driven]:**  
  While a configuration mutation is being processed, the mutation queue shall process exactly one mutation at a time in FIFO order (`CI-2`).
- **NFR-RL-03 [Unwanted Behavior]:**  
  If an operating system crash, power failure, or process kill occurs during a configuration mutation or restore swap, the system shall guarantee that `rclone.conf` is always in an old-or-new valid state and never partially written (`NF-4`).

---

### 4.5 Usability & Accessibility Principles (`NFR-AC`)

- **NFR-AC-01 [Ubiquitous]:**  
  The UI shall adhere to progressive disclosure (`UX-6`): essential controls and preset bundles shall be displayed by default, with advanced flags disclosed on demand.
- **NFR-AC-02 [Ubiquitous]:**  
  Every interactive widget in the native window shall expose AccessKit accessibility nodes with meaningful accessible names, roles, and values for screen readers (`NF-8`).
- **NFR-AC-03 [Ubiquitous]:**  
  Every primary workflow (adding a remote, mounting a drive, unmounting, browsing files, searching commands) shall be fully operable via keyboard shortcuts without requiring mouse input (`NF-8`).

---

## 5. Comprehensive Requirement Traceability Matrix

| Requirement ID | EARS Pattern | User Flow (`user-flows.md`) | SRDD ID (`rman-srdd.md`) | Verifying Test Case / Suite |
|---|---|---|---|---|
| **`FR-LC-01`** | Ubiquitous | Flow 1 (First Launch) | `DM-7`, `§7.15` | `bootstrap_tests::test_fr_lc_01_lc_03_nfr_th_01_auto_bootstrap_daemon_if_stopped` |
| **`FR-LC-02`** | Event-Driven | Flow 1 (First Launch) | `DM-10`, `§7.14` | `ipc_tests::test_fr_lc_02_nfr_sc_02_ipc_handshake_and_request_response` |
| **`FR-LC-03`** | Unwanted Behavior | Flow 1 (First Launch) | `DM-2`, `§7.1` | `bootstrap_tests::test_fr_lc_01_lc_03_nfr_th_01_auto_bootstrap_daemon_if_stopped` |
| **`FR-LC-04`** | State-Driven | Flow 1 (First Launch) | `BU-1`, `BU-4` | `supervisor_tests::test_fr_lc_04_binary_manager_layout_and_version_floor` |
| **`FR-LC-05`** | Unwanted Behavior | Flow 1 (First Launch) | `DM-5`, `DM-1` | `agent_tests::test_fr_lc_05_agent_ipc_service_endpoints` |
| **`FR-LC-06`** | Ubiquitous | Flow 1 (First Launch) | `DM-2`, `§9` | `platform_tests::test_fr_lc_06_paths_layout_structure` |
| **`FR-LC-07`** | State-Driven | Flow 1 (First Launch) | `DM-3` | `domain_tests::test_fr_lc_07_daemon_state_lifecycle` |
| **`FR-LC-08`** | Unwanted Behavior | Flow 1 (First Launch) | `DM-4` | `supervisor_tests::test_fr_lc_08_lc_09_lc_22_lc_23_crash_loop_breaker` |
| **`FR-LC-09`** | Unwanted Behavior | Flow 1 (First Launch) | `DM-4` | `supervisor_tests::test_fr_lc_08_lc_09_lc_22_lc_23_crash_loop_breaker` |
| **`FR-LC-10`** | Event-Driven | Flow 9 (Window Close) | `DM-7` | `ui_state_tests::test_fr_ui_03_ui_04_command_palette_and_dashboard_stats` |
| **`FR-LC-12`** | Event-Driven | Flow 9 (Reboot Reconnect) | `DM-8`, `§7.6` | `supervisor_tests::test_fr_lc_12_mt_13_reconciler_diff_and_actions` |
| **`FR-LC-14`** | Optional Feature | Flow 1 (Password Cmd) | `CF-8`, `§7.12` | `platform_tests::test_fr_lc_14_nfr_sc_03_credential_keyring_roundtrip` |
| **`FR-LC-15`** | Ubiquitous | Flow 1 (Log Buffer) | `DM-9`, `§7.15` | `supervisor_tests::test_fr_lc_15_log_ring_buffer_bounded` |
| **`FR-RM-01`** | Event-Driven | Flow 2 (Remote Wizard) | `CF-2` | `wizard_ui_tests::test_fr_rm_02_rm_03_wizard_provider_filter_and_selection` |
| **`FR-RM-02`** | Ubiquitous | Flow 2 (Remote Wizard) | `R3`, `CF-2` | `wizard_ui_tests::test_fr_rm_02_rm_03_wizard_provider_filter_and_selection` |
| **`FR-RM-03`** | Event-Driven | Flow 2 (Remote Wizard) | `CF-2` | `wizard_ui_tests::test_fr_rm_02_rm_03_wizard_provider_filter_and_selection` |
| **`FR-RM-04`** | Event-Driven | Flow 2 (Remote Wizard) | `CF-2` | `domain_tests::test_fr_rm_04_remote_name_validation` |
| **`FR-RM-05`** | Event-Driven | Flow 2 (Remote Wizard) | `R2`, `§7.4` | `client_tests::test_fr_rm_05_rm_08_wizard_driver_state_machine` |
| **`FR-RM-06`** | State-Driven | Flow 2 (Remote Wizard) | `R3`, `§7.3` | `form_engine_tests::test_fr_rm_06_form_engine_generation_from_option_schema` |
| **`FR-RM-08`** | Event-Driven | Flow 2 (Remote Wizard) | `R2`, `§7.4` | `client_tests::test_fr_rm_05_rm_08_wizard_driver_state_machine` |
| **`FR-RM-10`** | State-Driven | Flow 2 (OAuth Step) | `CF-3` | `client_tests::test_fr_rm_10_rm_12_rm_14_oauth_polling_and_cancellation` |
| **`FR-RM-12`** | State-Driven | Flow 2 (OAuth Step) | `CF-3` | `client_tests::test_fr_rm_10_rm_12_rm_14_oauth_polling_and_cancellation` |
| **`FR-RM-14`** | Event-Driven | Flow 2 (OAuth Cancel) | `CF-3` | `client_tests::test_fr_rm_10_rm_12_rm_14_oauth_polling_and_cancellation` |
| **`FR-RM-16`** | Event-Driven | Flow 2 (Dependency Graph) | `CF-6`, `CF-10` | `domain_tests::test_fr_rm_16_remote_dependency_graph` |
| **`FR-RM-18`** | Event-Driven | Flow 2 (Remote Deletion) | `CI-2`, `CF-6` | `config_tests::test_fr_rm_18_nfr_rl_02_mutation_queue_transaction` |
| **`FR-MT-01`** | Event-Driven | Flow 3 (Mount Creation) | `MT-1` | `wizard_ui_tests::test_fr_mt_02_mt_04_mount_modal_presets_mapping` |
| **`FR-MT-02`** | Ubiquitous | Flow 3 (Mount Presets) | `MT-2` | `domain_tests::test_fr_mt_02_mt_04_mount_presets_options` |
| **`FR-MT-03`** | Ubiquitous | Flow 3 (Drive Letters) | `MT-1` | `platform_tests::test_fr_mt_03_available_drive_letters` |
| **`FR-MT-04`** | Ubiquitous | Flow 3 (Mount Presets) | `MT-2` | `domain_tests::test_fr_mt_02_mt_04_mount_presets_options` |
| **`FR-MT-05`** | Unwanted Behavior | Flow 3 (Target Check) | `MT-4` | `domain_tests::test_fr_mt_05_windows_network_mode_validation` |
| **`FR-MT-06`** | State-Driven | Flow 3 (WinFsp Check) | `MT-4`, `IN-3` | `platform_tests::test_fr_mt_06_filesystem_driver_check` |
| **`FR-MT-07`** | Event-Driven | Flow 3 (Mount Execution) | `MT-1`, `MT-5` | `supervisor_tests::test_fr_lc_12_mt_13_reconciler_diff_and_actions` |
| **`FR-MT-13`** | State-Driven | Flow 4 (Unmanaged Mount) | `MT-7` | `supervisor_tests::test_fr_lc_12_mt_13_reconciler_diff_and_actions` |
| **`FR-SV-03`** | Ubiquitous | Flow 6 (Loopback Bind) | `SV-3` | `domain_tests::test_fr_sv_03_sv_04_serve_bind_auth_validation` |
| **`FR-SV-04`** | Unwanted Behavior | Flow 6 (LAN Auth) | `SV-3` | `domain_tests::test_sv_3_serve_profile_safe_bind_validation` |
| **`FR-FL-02`** | Event-Driven | Flow 5 (Files Listing) | `OT-1` | `client_tests::test_fr_lc_06_rc_client_core_calls` |
| **`FR-FL-07`** | Event-Driven | Flow 5 (Transfer Jobs) | `OT-2` | `domain_tests::test_fr_fl_07_job_profile_validation` |
| **`FR-BK-01`** | Event-Driven | Flow 7 (Snapshots) | `BK-1` | `config_tests::test_fr_bk_01_bk_04_snapshot_store_dedupe_and_retention` |
| **`FR-BK-02`** | State-Driven | Flow 7 (External Watch) | `CF-9` | `config_tests::test_fr_bk_02_config_watcher` |
| **`FR-BK-03`** | Event-Driven | Flow 7 (Redacted Diff) | `CI-5`, `BK-2` | `config_tests::test_fr_bk_03_nfr_sc_04_redacted_diff` |
| **`FR-BK-04`** | Ubiquitous | Flow 7 (Retention Pruning) | `BK-1` | `config_tests::test_fr_bk_01_bk_04_snapshot_store_dedupe_and_retention` |
| **`FR-BK-06`** | Event-Driven | Flow 7 (Whole Restore) | `BK-2`, `§7.5` | `config_tests::test_fr_bk_06_bk_07_nfr_rl_03_swap_restore_and_rollback` |
| **`FR-BK-07`** | Unwanted Behavior | Flow 7 (Rollback) | `BK-2`, `BK-4` | `config_tests::test_fr_bk_06_bk_07_nfr_rl_03_swap_restore_and_rollback` |
| **`FR-UI-01`** | Ubiquitous | Flow 8 (Navigation) | `UX-6`, `§7.11` | `tui_tests::test_fr_ui_01_ui_02_destination_switching_and_view_rendering` |
| **`FR-UI-02`** | Event-Driven | Flow 8 (Key Navigation) | `UX-2` | `tui_tests::test_fr_ui_01_ui_02_destination_switching_and_view_rendering` |
| **`FR-UI-03`** | Event-Driven | Flow 8 (Command Palette)| `UX-2` | `ui_state_tests::test_fr_ui_03_ui_04_command_palette_and_dashboard_stats` |
| **`FR-UI-04`** | State-Driven | Flow 8 (Search Filter) | `UX-2` | `ui_state_tests::test_fr_ui_03_ui_04_command_palette_and_dashboard_stats` |
| **`NFR-PF-01`**| Ubiquitous | Footprint Budget | `NF-13` | `xtask_tests::test_nfr_pf_01_command_catalog_and_footprint_budget` |
| **`NFR-PF-02`**| State-Driven | Memory Budget | `NF-1` | `agent_tests::test_fr_lc_05_agent_ipc_service_endpoints` |
| **`NFR-TH-01`**| Ubiquitous | Runtime Isolation | `§7.11` | `bootstrap_tests::test_fr_lc_01_lc_03_nfr_th_01_auto_bootstrap_daemon_if_stopped` |
| **`NFR-SC-03`**| Ubiquitous | Secret Handling | `§7.12` | `platform_tests::test_fr_lc_14_nfr_sc_03_credential_keyring_roundtrip` |
| **`NFR-SC-04`**| Ubiquitous | Redaction | `CI-5`, `UX-3` | `config_tests::test_fr_bk_03_nfr_sc_04_redacted_diff` |
| **`NFR-RL-01`**| Ubiquitous | Single Writer | `CI-1`, `R12` | `config_tests::test_nfr_rl_01_read_only_ini_parser` |
| **`NFR-RL-02`**| State-Driven | Mutation Queue | `CI-2` | `config_tests::test_fr_rm_18_nfr_rl_02_mutation_queue_transaction` |
| **`NFR-RL-03`**| Unwanted Behavior | Atomic Crash Safety | `NF-4` | `config_tests::test_fr_bk_06_bk_07_nfr_rl_03_swap_restore_and_rollback` |
