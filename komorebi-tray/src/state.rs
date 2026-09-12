use komorebi_client::Ring;
use serde::Deserialize;

// Read only the stable fields this app needs. Deserializing the complete client State
// would reject older daemons whenever unrelated required fields are added upstream.
#[derive(Deserialize)]
struct WorkspaceNotification {
    state: TrayState,
}

#[derive(Deserialize)]
struct TrayState {
    monitors: Ring<MonitorState>,
}

#[derive(Deserialize)]
struct MonitorState {
    #[serde(default)]
    name: String,
    workspaces: Ring<WorkspaceState>,
}

#[derive(Deserialize)]
struct WorkspaceState {
    name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayState {
    pub number: Option<usize>,
    pub tooltip: String,
}

impl DisplayState {
    pub fn disconnected() -> Self {
        Self {
            number: None,
            tooltip: "Komorebi: Disconnected".to_owned(),
        }
    }

    pub fn from_notification(bytes: &[u8]) -> serde_json::Result<Self> {
        let notification: WorkspaceNotification = serde_json::from_slice(bytes)?;
        Ok(Self::from_monitors(&notification.state.monitors))
    }

    fn from_monitors(monitors: &Ring<MonitorState>) -> Self {
        let Some((monitor, workspace)) = monitors.focused().and_then(|monitor| {
            monitor
                .workspaces
                .focused()
                .map(|workspace| (monitor, workspace))
        }) else {
            return Self {
                number: None,
                tooltip: "Komorebi: No focused workspace".to_owned(),
            };
        };

        let number = monitor.workspaces.focused_idx() + 1;
        let mut tooltip = format!("Komorebi: Workspace {number}");
        if let Some(name) = workspace.name.as_deref().filter(|name| !name.is_empty()) {
            tooltip.push_str(&format!(" — {name}"));
        }
        if !monitor.name.is_empty() {
            tooltip.push_str(&format!(" ({})", monitor.name));
        }
        Self {
            number: Some(number),
            tooltip,
        }
    }

    pub fn label(&self) -> String {
        self.number
            .map_or_else(|| "—".to_owned(), |number| number.to_string())
    }
}

/// Leave room for the terminator without splitting a UTF-16 surrogate pair.
pub fn tooltip_utf16(text: &str) -> [u16; 128] {
    let mut result = [0; 128];
    let mut length = 0;
    for character in text.chars() {
        let character = if character.is_control() {
            ' '
        } else {
            character
        };
        let mut buffer = [0; 2];
        let encoded = character.encode_utf16(&mut buffer);
        if length + encoded.len() >= result.len() {
            break;
        }
        result[length..length + encoded.len()].copy_from_slice(encoded);
        length += encoded.len();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(name: &str, focused: usize) -> MonitorState {
        let mut monitor = MonitorState {
            name: String::new(),
            workspaces: Ring::default(),
        };
        monitor.name = name.to_owned();
        monitor
            .workspaces
            .elements_mut()
            .resize_with(12, || WorkspaceState { name: None });
        monitor.workspaces.focus(focused);
        monitor
    }

    #[test]
    fn follows_focused_monitor_with_one_based_numbers() {
        let mut monitors = Ring::default();
        monitors.elements_mut().push_back(monitor("Left", 0));
        monitors.elements_mut().push_back(monitor("Right", 11));
        assert_eq!(DisplayState::from_monitors(&monitors).label(), "1");
        monitors.focus(1);
        let display = DisplayState::from_monitors(&monitors);
        assert_eq!(display.label(), "12");
        assert_eq!(display.tooltip, "Komorebi: Workspace 12 (Right)");
    }

    #[test]
    fn names_do_not_replace_numbers_and_monitor_changes_update_tooltips() {
        let mut monitors = Ring::default();
        let mut left = monitor("Left", 2);
        left.workspaces.focused_mut().unwrap().name = Some("Code".to_owned());
        monitors.elements_mut().push_back(left);
        monitors.elements_mut().push_back(monitor("Right", 2));
        let left = DisplayState::from_monitors(&monitors);
        assert_eq!(left.tooltip, "Komorebi: Workspace 3 — Code (Left)");
        monitors.focus(1);
        let right = DisplayState::from_monitors(&monitors);
        assert_eq!(left.number, right.number);
        assert_ne!(left, right);
    }

    #[test]
    fn invalid_selections_never_display_a_fabricated_number() {
        let mut monitors = Ring::default();
        assert_eq!(DisplayState::from_monitors(&monitors).number, None);
        monitors.elements_mut().push_back(monitor("Left", 12));
        assert_eq!(DisplayState::from_monitors(&monitors).number, None);
        monitors.focus(99);
        assert_eq!(DisplayState::from_monitors(&monitors).number, None);
        assert_eq!(DisplayState::disconnected().label(), "—");
    }

    #[test]
    fn accepts_older_daemons_and_ignores_unrelated_state_fields() {
        let bytes = br#"{
            "event": {"type": "FutureEvent"},
            "state": {
                "monitors": {
                    "focused": 0,
                    "elements": [{
                        "name": "DISPLAY1",
                        "future_monitor_option": true,
                        "workspaces": {
                            "focused": 1,
                            "elements": [{"name":"I"}, {"name":"II", "layout":"BSP"}]
                        }
                    }]
                },
                "unrelated_option": "ignored"
            }
        }"#;
        let display = DisplayState::from_notification(bytes).unwrap();
        assert_eq!(display.label(), "2");
        assert_eq!(display.tooltip, "Komorebi: Workspace 2 — II (DISPLAY1)");
    }

    #[test]
    fn malformed_notifications_are_rejected() {
        for bytes in [b"not json".as_slice(), b"{}", b"{\"state\":null}", b"\xff"] {
            assert!(DisplayState::from_notification(bytes).is_err());
        }
    }

    #[test]
    fn tooltip_is_terminated_and_preserves_unicode() {
        let text = format!("{}🚀", "a".repeat(126));
        let tooltip = tooltip_utf16(&text);
        assert_eq!(tooltip[126], 0);
        assert!(String::from_utf16(&tooltip[..126]).is_ok());
        let text = format!("{}🚀", "a".repeat(125));
        let tooltip = tooltip_utf16(&text);
        assert_eq!(String::from_utf16(&tooltip[..127]).unwrap(), text);
        assert_eq!(tooltip[127], 0);
        assert_eq!(tooltip_utf16("a\0b\nc")[..5], [97, 32, 98, 32, 99]);
    }
}
