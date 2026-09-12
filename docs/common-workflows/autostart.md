# Autostart

If you would like to autostart `komorebi`, you can use the `komorebic enable-autostart` command to generate a shortcut
in the `shell:startup` folder.

Add `--tray` to start the workspace tray indicator at login, for example:

```powershell
komorebic enable-autostart --whkd --tray
```

`--tray` can be combined with `--bar`. Both are optional. The shortcut uses
`komorebic-no-console`, so no console window opens at login. Run
`komorebic disable-autostart` to remove the shared startup shortcut.

```
Generates the komorebi.lnk shortcut in shell:startup to autostart komorebi

Usage: enable-autostart [OPTIONS]

Options:
  -c, --config <CONFIG>
          Path to a static configuration JSON file

      --whkd
          Enable autostart of whkd

      --bar
          Enable autostart of komorebi-bar

      --tray
          Enable autostart of komorebi-tray

      --masir
          Enable autostart of masir

  -h, --help
          Print help

```