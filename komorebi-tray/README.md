# Komorebi Tray

A Windows tray indicator for the selected workspace on Komorebi's focused monitor.
The first workspace displays **1**. When focus moves between monitors, the indicator
follows it. Hover over the number to see the workspace name and monitor name.

## Build and run

From the repository root on Windows with the repository's Rust toolchain:

```powershell
cargo build -p komorebi-tray --release --locked
Start-Process .\target\release\komorebi-tray.exe -WindowStyle Hidden
```

You can copy the release .exe to another directory and double-click it there.
No additional files are needed; the app opens only a tray icon.

Or install the binary into Cargo's bin directory:

```powershell
cargo install --path komorebi-tray --locked
komorebi-tray.exe
```

The existing `just build`, `just install`, and `just copy` recipes include this app.
It runs without a console window or configuration file. Start it manually; it does
not register itself for Windows startup or start Komorebi. Launching it a second
time in the same Windows session leaves the existing instance running.

Windows may initially put the icon in the tray overflow menu. Drag it into the
visible notification area, or enable it in Windows' taskbar tray settings.

Right-click the icon and select **Exit** to close the indicator. It does not change
workspaces, monitor focus, or Komorebi configuration.

## Connection and display

- Workspace numbers update from the same state notifications used by `komorebi-bar`.
- `—` means there is no current workspace to display. The tooltip distinguishes
  a disconnected daemon from an unavailable focused workspace.
- If Komorebi is unavailable at launch or stops, the app stays running and retries
  every second. After five seconds without notifications it refreshes its
  subscription, recovering from silent daemon restarts too.
- The icon is restored after Explorer restarts and rendered for the taskbar's DPI.
- Workspace names do not replace numeric labels. Long tooltip text is truncated
  at the Windows tooltip limit without breaking Unicode characters.

## Development checks

```powershell
cargo +nightly fmt -p komorebi-tray --check
cargo clippy -p komorebi-tray --all-targets -- -D warnings
cargo test -p komorebi-tray
```

Manual acceptance checks: change workspace and monitor focus, start the indicator
before Komorebi, restart Komorebi, recreate the taskbar, open/dismiss the Exit menu,
and verify the icon at 100%, 150%, and 200% scaling on light and dark taskbars.
