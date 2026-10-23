use serde_json::{Map, Value};
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct Preferences {
    values: Map<String, Value>,
    path: Option<PathBuf>,
    remote_settings: Option<Value>,
}

const KEYS: &[(&str, &str)] = &[
    ("panelLayout", "panel_layout"),
    ("displayUnit", "display_unit"),
    ("speedTimeUnit", "speed_time_unit"),
    ("uiTheme", "ui_theme"),
    ("darkMode", "dark_mode"),
    ("antialiasing", "antialiasing"),
    ("artworkDisplayMode", "artwork_display_mode"),
    ("transformAnchor", "transform_anchor"),
    ("reduceMotion", "reduce_motion"),
    ("showPaletteLabels", "show_palette_labels"),
    ("cursorSize", "cursor_size"),
    ("toolbarIconSize", "toolbar_icon_size"),
    ("clickTolerancePx", "click_tolerance_px"),
    ("snapThresholdPx", "snap_threshold_px"),
    ("gridSpacingMm", "grid_spacing_mm"),
    ("nudgeStepMm", "nudge_step_mm"),
    ("nudgeStepFineMm", "nudge_step_fine_mm"),
    ("nudgeStepCoarseMm", "nudge_step_coarse_mm"),
    ("scrollZoom", "scroll_zoom"),
    ("customHotkeys", "custom_hotkeys"),
    ("displayLanguage", "display_language"),
];

impl Preferences {
    pub fn load(path: PathBuf) -> Result<Self, String> {
        let values = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Map<String, Value>>(&bytes)
                .map_err(|_| "Invalid local UI settings JSON")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
            Err(e) => return Err(format!("Cannot read local UI settings: {e}")),
        };
        if values
            .keys()
            .any(|key| !KEYS.iter().any(|(_, stored)| key == stored))
        {
            return Err("Local UI settings contain unsupported fields".into());
        }
        Ok(Self {
            values,
            path: Some(path),
            remote_settings: None,
        })
    }

    pub fn validate_patch(args: &Value) -> Result<Map<String, Value>, String> {
        let args = args.as_object().ok_or("UI settings must be an object")?;
        let mut patch = Map::new();
        for (key, value) in args {
            let key = KEYS
                .iter()
                .find(|(input, _)| key == input)
                .map(|(_, stored)| *stored)
                .ok_or_else(|| {
                    format!("Remote prototype is read-only for server settings: {key}")
                })?;
            if key == "panel_layout" && !value.is_object() && !value.is_null() {
                return Err("panelLayout must be an object or null".into());
            }
            patch.insert(key.into(), value.clone());
        }
        Ok(patch)
    }

    pub fn cached_remote(&self) -> Option<Value> {
        self.remote_settings.clone()
    }

    pub fn merge(&mut self, mut settings: Value) -> Result<Value, String> {
        let object = settings
            .as_object_mut()
            .ok_or("Invalid server settings response")?;
        self.remote_settings = Some(Value::Object(object.clone()));
        object.extend(self.values.clone());
        Ok(settings)
    }

    pub fn update(&mut self, patch: Map<String, Value>, settings: Value) -> Result<Value, String> {
        if !settings.is_object() {
            return Err("Invalid server settings response".into());
        }
        let mut values = self.values.clone();
        values.extend(patch);
        if let Some(path) = &self.path {
            let parent = path.parent().ok_or("Invalid preferences path")?;
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create local UI directory: {e}"))?;
            let temporary = path.with_extension("tmp");
            let json =
                serde_json::to_vec_pretty(&values).map_err(|_| "Cannot encode UI settings")?;
            std::fs::write(&temporary, json)
                .and_then(|_| std::fs::rename(&temporary, path))
                .map_err(|e| format!("Cannot save local UI settings: {e}"))?;
        }
        self.values = values;
        self.merge(settings)
    }
}
