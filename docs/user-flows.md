# Rclone Manager (RCM) — Comprehensive User Flows & Interaction Specification

| | |
|---|---|
| **Document** | Detailed User Flows & Interaction Guide |
| **App Title** | Rclone Manager ("RCM" / `rcm.exe`) |
| **Reference** | Companion to Software Requirements & Design Specification (`rman-srdd.md`) |
| **Target Audience** | Users, UI/UX designers, QA engineers, and developers |

---

## 1. Introduction & Personas

Rclone Manager (RCM) provides a zero-friction desktop control plane over `rclone`. The interface adheres to the core principle: **"Minimal and lightweight, without cutting features"** (§5.1).

### Personas

- **Everyday User ("Alice"):** Wants her Google Drive or Dropbox mounted as drive `G:` in three clicks, auto-reconnecting on system reboot, without touching terminals, scripts, or configuration files.
- **Power User / Sysadmin ("Bob"):** Manages multiple cloud backends (AWS S3, SFTP, Crypt wrappers), custom VFS cache tuning, automated sync jobs, and hand-maintained `rclone.conf` files with automated rollback protection.
- **Homelab User ("Charlie"):** Exposes cloud storage over WebDAV or SFTP to home devices (smart TVs, mobile apps, media players), requiring reliable network binds and security defaults.

---

## 2. User Flow 1: First-Time Launch & Zero-Friction Setup

```
[ User Double-Clicks rcm.exe ]
               │
               ▼
   Is rcm-agent running? ──(No)──► Silently spawn rcm-agent.exe in background
               │ (Yes)
               ▼
   Is rclone binary present? ──(No)──► Auto-download latest verified rclone
               │ (Yes)
               ▼
   Is rcd daemon running? ──(No)──► Spawn rcd with random loopback credentials
               │ (Yes)
               ▼
[ Native GPUI Window Opens Instantly (1080x720) ]
```

### The User Story
*As a new user, I want to double-click `rcm.exe` and have the app open ready to use, without having to start background daemons or run setup scripts in a terminal.*

### Step-by-Step Interaction

1. **Launch:**
   - Alice downloads `rcm.exe` (or runs it from `target\release\rcm.exe`).
   - Alice double-clicks `rcm.exe` in Windows File Explorer.
2. **What Happens Behind the Scenes (Zero Friction):**
   - `rcm.exe` immediately checks for the local IPC named pipe (`\\.\pipe\rcm-<username>`).
   - Detecting that `rcm-agent` is not running, `rcm.exe` automatically spawns `rcm-agent.exe --background` using Windows `CREATE_NO_WINDOW` (preventing any annoying black console window flash).
   - `rcm-agent` initializes, checks `%LOCALAPPDATA%\RCM\rclone\`, and detects that no rclone executable is present.
   - `rcm-agent` queries official release metadata, downloads the verified stable release (`rclone-v1.75.1-windows-amd64.zip`), checks that it meets the security floor ($\ge$ v1.73.5), extracts it, and registers it.
   - `rcm-agent` spawns `rclone rcd` with per-session random credentials on a dynamically chosen loopback port.
3. **What the User Sees:**
   - The native GPUI hardware-accelerated desktop window appears centered on screen.
   - The top header bar displays a live green indicator:  
     `● rclone v1.75.1 [READY] (PID 88500 • http://127.0.0.1:52356)`
   - The **Dashboard** is presented with three metric cards:
     - **Active Mounts:** `0`
     - **Active Serves:** `0`
     - **Configured Remotes:** `0`
   - Three primary action buttons are visible:
     - `💽 + New Mount`
     - `☁ + Add Remote`
     - `⚡ Reconcile Mounts`
   - The footer confirms: `Ready. Daemon running in background.`

---

## 3. User Flow 2: Adding a Cloud Remote (Interactive CLI-Parity Wizard)

### The User Story
*As Alice, I want to connect my Google Drive using an intuitive graphical wizard that guides me through authentication and opens my browser to grant access.*

### Step-by-Step Interaction

```
[ Click + Add Remote ]
          │
          ▼
[ Step 1: Select Provider & Name ]
  • Pick: Google Drive
  • Name: my_gdrive
  • Click: Continue to Configuration →
          │
          ▼
[ Step 2: Dynamic Question Flow ]
  • Client ID (leave blank for default)
  • Client Secret (leave blank for default)
  • Scope: Full Access [Dropdown/Button]
          │
          ▼
[ Step 3: OAuth Browser Authorization ]
  • Click: 🌐 Open Auth in Browser
  • Browser opens Google consent page
  • Alice clicks "Allow"
  • Daemon automatically receives OAuth token
          │
          ▼
[ Step 4: Finish Setup ]
  • Click: Finish Setup ✓
  • New remote appears in Remotes list with "cloud" badge
```

1. **Initiate Wizard:**
   - Alice clicks the `☁ + Add Remote` button on the Dashboard (or clicks `[2] Remotes` in the sidebar, then clicks `+ New Remote Wizard`).
   - A dark modal overlay smoothly slides into view titled:  
     **"New Remote Wizard — Select Cloud Backend (Step 1 of 2)"**.
2. **Select Provider and Name:**
   - Alice sees a list of supported cloud backends:
     - **Google Drive** — *Cloud storage by Google*
     - **Amazon S3** — *S3 compliant object storage*
     - **Dropbox** — *Dropbox cloud sync*
     - **Microsoft OneDrive** — *OneDrive personal / business*
     - **WebDAV** — *Generic WebDAV server*
     - **SFTP** — *SSH file transfer*
     - **Encrypt (Crypt)** — *Client-side encryption wrapper*
   - Alice clicks the **"Select"** button next to **Google Drive**. The row highlights in soft blue with a `Selected ✓` badge.
   - In the **Remote Name** selector, Alice clicks `my_gdrive` (or enters a custom name).
   - Alice clicks **`Continue to Configuration →`**.
3. **Dynamic Question Flow (CLI-Parity via `WizardDriver`):**
   - The modal updates to: **"Configure Backend (Step 2 of 2)"**.
   - Behind the scenes, RCM sends `config/create` with `nonInteractive=true` to the daemon.
   - rclone returns its first question: `client_id`.
   - The modal displays:
     - **Title:** `client_id`
     - **Help:** `Google Application Client Id. Leave blank normally.`
   - Alice leaves it blank for the built-in default and clicks **`Continue →`**.
   - rclone asks `client_secret`.
   - The password field is masked with bullet points. Alice clicks **`Continue →`**.
   - rclone asks `scope`. A selector offers options: `1: Full access (drive)`, `2: Read-only (drive.readonly)`. Alice selects `1`.
4. **OAuth Browser Step:**
   - rclone begins OAuth server mode and emits an authentication URL.
   - The wizard modal detects this and displays:
     - A banner: **"OAuth Browser Authentication Required"**
     - URL: `http://127.0.0.1:53682/auth?state=...`
     - A prominent button: **`🌐 Open Auth in Browser`**
   - Alice clicks **`🌐 Open Auth in Browser`**.
   - Windows automatically opens her default web browser (Chrome/Edge/Firefox) to Google's sign-in page.
   - Alice signs in and clicks **"Allow"** to grant access to rclone.
   - The browser displays: *"Success! All done. Please go back to rclone."*
   - Simultaneously, RCM's background polling receives the completion signal.
5. **Completion:**
   - The wizard modal shows a green checkmark: **"Remote 'my_gdrive' configured successfully!"**
   - Alice clicks **`Finish Setup ✓`**.
   - The modal closes. RCM takes an automated `pre-mutation` snapshot of `rclone.conf`, clears `fscache`, and updates the UI.
   - `my_gdrive` now appears in the **Remotes** list with a `cloud` badge and action buttons: `📁 Browse` and `🗑 Delete`.

---

## 4. User Flow 3: Mounting Cloud Storage as a Windows Drive Letter

### The User Story
*As Alice, I want my Google Drive to appear in Windows File Explorer as drive `G:`, so I can open, edit, and save files just like a local hard drive.*

### Step-by-Step Interaction

```
[ Click 💽 + New Mount ]
          │
          ▼
[ Mount Profile Dialog ]
  • Pick Remote: my_gdrive
  • Pick Drive Letter: G:
  • Pick Preset: Balanced (or Streaming)
  • Click: Save & Mount as G:
          │
          ▼
[ Mount Reconciled & Created ]
  • Card displays: ● Mounted (G:)
  • Click 📂 Open ──► Windows Explorer opens G:\
```

1. **Open Mount Dialog:**
   - Alice clicks `💽 + New Mount` on the Dashboard (or navigates to `[3] Mounts` and clicks `+ New Mount Profile`).
   - A modal dialog appears: **"Create New Mount Profile"**.
2. **Configure Mount:**
   - **Select Cloud Remote:** Alice sees her configured remotes (`my_gdrive`). She clicks `● my_gdrive`.
   - **Target Drive Letter:** Alice sees available drive buttons: `G:`, `M:`, `Z:`, `X:`, `Y:`, `P:`. She clicks `G:`.
   - **Mount Preset:**
     - Alice sees two preset options with plain-language explanations:
       - **`Balanced (writes cache)`** — Recommended for everyday office work, editing documents, and saving files. Uses `vfs_cache_mode=writes` with a 30-minute directory cache.
       - **`Streaming (full cache)`** — Recommended for video playback, large media files, and offline access. Uses `vfs_cache_mode=full` with 256MB read-ahead.
     - Alice clicks **`Balanced`**.
3. **Save and Mount:**
   - Alice clicks **`Save & Mount as G:`**.
   - RCM:
     1. Writes the profile to `%APPDATA%\RCM\profiles.toml` via IPC.
     2. Calls `mount/mount` on `rcd` with parameters: `fs="my_gdrive:"`, `mountPoint="G:"`, `mountType="cmount"`, `vfs_cache_mode="writes"`.
     3. WinFsp allocates drive `G:` in the Windows shell.
   - The modal closes.
4. **Interacting with the Mounted Drive:**
   - In the **Mounts** list, the card now displays:  
     `G:   my_gdrive Drive (my_gdrive)   [Balanced]   ● Mounted`
   - Alice clicks the **`📂 Open`** button on the card.
   - Windows File Explorer opens immediately at `G:\`, displaying all her Google Drive files and folders as if they were stored on an external hard drive!
   - Alice can double-click a Word document, edit it, save it, and rclone transparently caches and uploads the changes in the background.

---

## 5. User Flow 4: Unmounting, Remounting & Drive Management

### The User Story
*As Alice, I want to temporarily disconnect drive `G:` or reconnect it later with a single click.*

### Step-by-Step Interaction

1. **Unmounting:**
   - Alice opens RCM and navigates to `[3] Mounts`.
   - On the `G:` card, she clicks **`⏹ Unmount`**.
   - RCM sends `mount/unmount` with `mountPoint="G:"` to rclone.
   - Drive `G:` disappears from Windows File Explorer cleanly without any explorer hangs or frozen windows.
   - The status badge on the card flips to `○ Stopped`.
   - The action buttons change to `▶ Mount` and `🗑 Delete`.
2. **Remounting:**
   - Later, Alice wants her drive back. She clicks **`▶ Mount`**.
   - Drive `G:` re-attaches within 1 second. The badge turns green (`● Mounted`) and the `📂 Open` button returns.

---

## 6. User Flow 5: Remote File Explorer (Browsing Without Mounting)

### The User Story
*As Bob, I want to inspect files in an S3 bucket or Google Drive without assigning a drive letter or installing WinFsp.*

### Step-by-Step Interaction

```
[ Click [5] Files in Sidebar ]
          │
          ▼
[ Select Remote Button: my_gdrive ]
          │
          ▼
[ Directory Table Loads ]
  📁 Documents      DIR         2026-10-05 10:15
  📁 Photos         DIR         2026-10-04 18:22
  📄 budget.xlsx    142.5 KB    2026-10-05 09:30
  📄 report.pdf     1.2 MB      2026-10-03 14:10
```

1. **Navigate to Files View:**
   - Bob clicks `[5] Files` in the sidebar (or clicks `📁 Browse` on any remote card in the Remotes view).
2. **Select Remote:**
   - At the top of the screen, Bob sees remote selector pills (`[▶ my_gdrive]`, `[my_s3]`).
   - Bob clicks `my_gdrive`.
3. **Directory Listing:**
   - RCM calls `operations/list` on `rcd` for `my_gdrive:`.
   - A clean table displays all directory contents:
     - Folders marked with `📁` and type `DIR`.
     - Files marked with `📄`, showing formatted sizes (e.g. `142.5 KB`, `1.2 MB`) and last-modified timestamps.
   - The view is fast, virtualized, and responsive.

---

## 7. User Flow 6: Exposing Cloud Storage Over LAN (Serves)

### The User Story
*As Charlie, I want to share my cloud media folder over my local home network via WebDAV so my smart TV and iPad can stream movies.*

### Step-by-Step Interaction

1. **Navigate to Serves View:**
   - Charlie clicks `[4] Serves` in the sidebar.
2. **Review Available Protocols:**
   - Charlie reviews available server protocols:
     - **WebDAV:** Stream files to Infuse, VLC, or mobile file managers.
     - **SFTP:** Secure shell file transfer endpoint.
     - **HTTP:** Browser-accessible download directory.
     - **S3:** S3-compatible gateway.
3. **Start a Serve:**
   - Charlie clicks `+ New Serve`.
   - Selects remote: `my_gdrive:Movies`.
   - Protocol: `WebDAV`.
   - Address: `0.0.0.0:8080`.
   - Because the address is non-loopback (`0.0.0.0`), RCM enforces authentication per requirement `SV-3`: Charlie enters username `charlie` and a password.
   - Charlie clicks **`Start WebDAV Serve`**.
4. **Connection:**
   - The card shows: `WEBDAV   my_gdrive:Movies on 0.0.0.0:8080   ● Running`.
   - Charlie opens VLC on his Apple TV, types `http://192.168.1.50:8080`, enters his credentials, and streams videos directly from his cloud account.
5. **Stopping:**
   - When finished, Charlie clicks **`Stop`** on the serve card. The endpoint closes immediately.

---

## 8. User Flow 7: Automated Backups, Redacted Diffs & Safe Config Restore

### The User Story
*As Bob, I want to be 100% confident that modifying a remote or changing configuration will never corrupt my `rclone.conf`, and I want one-click rollback if something goes wrong.*

### Step-by-Step Interaction

```
[ Bob edits or creates a remote in RCM ]
                     │
                     ▼
  Agent checks SHA-256 hash of rclone.conf
                     │
                     ▼
  Content changed? ──(Yes)──► Automated snapshot created:
                              rclone.conf.1759683100.pre-mutation.a8f3b92c
                     │
                     ▼
  Mutation executed via RC (rcd is the sole writer)
```

1. **Automatic Snapshot Creation:**
   - Whenever a remote is created, updated, or deleted, RCM automatically compares the SHA-256 hash of `rclone.conf` against the latest snapshot.
   - If changed, a snapshot is saved to `%LOCALAPPDATA%\RCM\backups\`:
     `rclone.conf.<timestamp>.pre-mutation.<hash8>`.
2. **Inspecting Backups & Diffs:**
   - Bob navigates to `[6] Settings`.
   - In the **Backups & Snapshots** section, Bob sees a list of snapshots with reasons (`pre-mutation`, `scheduled`, `external-change`, `manual`).
   - Bob clicks **`View Diff`** on a snapshot.
   - A modal displays the unified diff:
     - Normal settings show concrete line differences.
     - All sensitive keys (`secret_access_key`, `password`, `token`) are automatically masked as `***REDACTED***` (`CI-5`).
3. **Safe Whole-File Restore (Stop $\rightarrow$ Swap $\rightarrow$ Start per §7.5):**
   - Bob wants to revert to yesterday's snapshot.
   - Bob clicks **`Restore Snapshot`**.
   - RCM:
     1. Takes a `pre-restore` snapshot of the current configuration.
     2. Gracefully stops `rcd` (`mount/unmountall`, `core/quit`).
     3. Atomically replaces `rclone.conf` with the chosen snapshot.
     4. Starts `rcd` and verifies readability.
     5. Re-establishes all active mounts and serves.
   - Bob's previous configuration is fully restored without any data loss or partial writes.

---

## 9. User Flow 8: Global Command Palette (`Ctrl+K`)

### The User Story
*As a power user who prefers the keyboard, I want to jump to any remote, mount, or settings page using a quick search palette.*

### Step-by-Step Interaction

1. **Trigger:**
   - From any screen in RCM, Bob presses **`Ctrl+K`**.
   - An overlay palette slides into the center of the window:  
     `Search Commands: _`
2. **Search:**
   - Bob types `mount`.
   - The palette instantly filters results:
     - `> Go to Mounts: Drive letters and VFS profiles`
     - `  Reconcile Mounts: Trigger sync of active virtual drives`
3. **Execute:**
   - Bob presses **`Enter`**.
   - The palette dismisses, and RCM navigates directly to the Mounts destination.

---

## 10. User Flow 9: Exiting the Window & Resident Background Operation

### The User Story
*As Alice, when I close the RCM window, I want my drive `G:` to stay mounted so my office apps don't crash, and I want everything to automatically reconnect when I restart my PC.*

### Step-by-Step Interaction

1. **Closing the UI Window:**
   - Alice finishes configuring her drives and clicks the `[X]` close button in the top-right corner of the window (or presses `q`).
   - The native GPUI window closes immediately, freeing all GPU and DirectX rendering memory (`DM-7`).
   - Alice checks Windows File Explorer: **Drive `G:` is still mounted and completely accessible!**
   - The lightweight headless `rcm-agent.exe` remains running in the background, consuming $\le$ 30MB RSS, monitoring `rcd` health every 5 seconds.
2. **Reopening the App:**
   - Later, Alice double-clicks `rcm.exe` again.
   - `rcm.exe` detects the existing resident agent, connects to the named pipe within milliseconds, and displays the UI with all current live stats already loaded.
3. **System Reboot (Autostart per `DM-6`):**
   - Alice shuts down her computer for the night and boots it up the next morning.
   - At Windows login, `rcm-agent.exe --background` starts automatically via HKCU Run.
   - `rcm-agent` starts `rcd`, reads `profiles.toml`, and immediately mounts drive `G:` in the background.
   - Alice opens Windows File Explorer: drive `G:` is already waiting for her before she even opens RCM!

---

## 11. Summary Matrix of User Interactions

| Action | Where in UI | What the User Clicks / Types | What Happens Behind the Scenes |
|---|---|---|---|
| **First Launch** | Desktop / Start Menu | Double-click `rcm.exe` | Auto-spawns agent, auto-provisions verified rclone binary if missing, starts `rcd`, renders native GPU window. |
| **Add Remote** | Dashboard or Remotes | `+ New Remote Wizard` $\rightarrow$ pick backend $\rightarrow$ enter name | Runs non-interactive state machine via `WizardDriver`, guides through questions, handles OAuth, saves section. |
| **Authorize OAuth** | Remote Wizard Step 2 | `🌐 Open Auth in Browser` | Opens default web browser to Google/Dropbox consent page; agent captures token automatically. |
| **Create Mount** | Dashboard or Mounts | `+ New Mount Profile` $\rightarrow$ pick drive letter & preset $\rightarrow$ Save | Saves profile to `profiles.toml`, calls `mount/mount`, WinFsp creates drive in Windows Explorer. |
| **Open in Explorer** | Mounts Tab | `📂 Open` | Opens `explorer.exe <drive>:\` directly. |
| **Unmount Drive** | Mounts Tab | `⏹ Unmount` | Calls `mount/unmount`, drive letter unmounts cleanly. |
| **Browse Files** | Remotes or Files Tab | `📁 Browse` on remote card | Calls `operations/list`, renders file table with sizes and timestamps. |
| **Start WebDAV** | Serves Tab | `+ New Serve` $\rightarrow$ WebDAV $\rightarrow$ Start | Validates loopback/LAN auth (`SV-3`), calls `serve/start`, displays copyable endpoint URL. |
| **Restore Backup** | Settings Tab | `View Diff` $\rightarrow$ `Restore Snapshot` | Redacts sensitive secrets, pauses `rcd`, replaces `rclone.conf` atomically, restarts and reconciles. |
| **Command Search** | Anywhere in App | Press `Ctrl+K` $\rightarrow$ type query $\rightarrow$ `Enter` | Filters all actions and destinations with keyboard navigation. |
| **Close App** | Titlebar / Key `q` | Click `[X]` or press `q` | Window closes; background agent and mounted drives continue running uninterrupted. |
| **Reboot System** | Windows Desktop | Turn PC off and on | Agent autostarts via registry, re-mounts all autostart drive letters automatically at login. |
