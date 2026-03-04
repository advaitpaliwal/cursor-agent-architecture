use std::collections::HashMap;

use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CursorType {
    Unspecified = 0,
    Arrow = 1,
    Pointer = 2,
    Text = 3,
    Wait = 4,
    Crosshair = 5,
    Move = 6,
    ResizeNs = 7,
    ResizeEw = 8,
    ResizeNwse = 9,
    ResizeNesw = 10,
    NotAllowed = 11,
    Grab = 12,
    Grabbing = 13,
}

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum MotionStyle {
    Unspecified = 0,
    Slow = 1,
    Mellow = 2,
    Quick = 3,
    Rapid = 4,
}

impl Default for MotionStyle {
    fn default() -> Self {
        Self::Unspecified
    }
}

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum IdleClassification {
    Unspecified = 0,
    LoadingWait = 1,
    ViewingResult = 2,
    ThinkingPause = 3,
    LongOperation = 4,
}

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ClickType {
    Unspecified = 0,
    Single = 1,
    Double = 2,
    Triple = 3,
    Right = 4,
    Middle = 5,
}

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum KeystrokeEventType {
    Unspecified = 0,
    KeyCombo = 1,
    KeySingle = 2,
    TextTyped = 3,
}

#[derive(Debug, Copy, Clone, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum MouseButton {
    Unspecified = 0,
    Left = 1,
    Right = 2,
    Middle = 3,
    Back = 4,
    Forward = 5,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackPlan {
    #[serde(default)]
    pub segments: Vec<PlaybackSegment>,
    pub output_duration_ms: f64,
    #[serde(default)]
    pub source_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSegment {
    #[serde(rename = "type")]
    pub segment_type: SegmentType,
    pub source_start_ms: f64,
    pub source_end_ms: f64,
    pub source_duration_ms: f64,
    pub output_start_ms: f64,
    pub output_end_ms: f64,
    pub output_duration_ms: f64,
    pub playback_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SegmentType {
    Action,
    Gap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTracks {
    #[serde(default)]
    pub click_effects: Vec<ClickEffectKeyframe>,
    #[serde(default)]
    pub keystroke_events: Vec<KeystrokeEvent>,
    #[serde(default)]
    pub zoom_windows: Vec<ZoomWindow>,
    pub cursor_style: MotionStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDiagnostics {
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    pub alignment_delta_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoMeta {
    pub input_video_path: String,
    pub source_duration_ms: f64,
    pub output_duration_ms: f64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub config_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPlan {
    pub video: VideoMeta,
    pub playback: PlaybackPlan,
    pub tracks: RenderTracks,
    #[serde(default)]
    pub decision_input: DecisionInput,
    #[serde(default)]
    pub decisions: DecisionOutput,
    pub diagnostics: PlanDiagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DecisionInput {
    #[serde(default)]
    pub video_path: String,
    #[serde(default)]
    pub video_duration_ms: f64,
    #[serde(default)]
    pub video_width: u32,
    #[serde(default)]
    pub video_height: u32,
    #[serde(default)]
    pub action_count: u32,
    #[serde(default)]
    pub zoom_candidates: Vec<ZoomCandidate>,
    #[serde(default)]
    pub idle_periods: Vec<IdlePeriod>,
    #[serde(default)]
    pub click_effects: Vec<ClickEffectKeyframe>,
    #[serde(default)]
    pub keystroke_events: Vec<KeystrokeEvent>,
    #[serde(deserialize_with = "deserialize_cursor_paths", default)]
    pub cursor_paths: Vec<CursorPath>,
    #[serde(default)]
    pub zoom_windows: Vec<ZoomWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DecisionOutput {
    pub cursor_style: MotionStyle,
    #[serde(default)]
    pub selected_zooms: Vec<ZoomSelection>,
    #[serde(default)]
    pub selected_speedups: Vec<SpeedupSelection>,
    pub show_click_effects: bool,
    #[serde(default)]
    pub selected_click_effects: Vec<usize>,
    pub show_keystrokes: bool,
    #[serde(default)]
    pub cuts: Vec<VideoCut>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomSelection {
    pub candidate_index: usize,
    pub zoom_override: Option<f64>,
    pub start_ms_override: Option<f64>,
    pub end_ms_override: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedupSelection {
    pub candidate_index: usize,
    pub speed_override: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCut {
    pub start_ms: f64,
    pub end_ms: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coordinate {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomCandidate {
    pub start_ms: f64,
    pub end_ms: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub suggested_zoom: f64,
    pub action_type: String,
    pub action_index: i64,
    pub importance_score: f64,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomWindow {
    pub start_ms: f64,
    pub end_ms: f64,
    #[serde(default)]
    pub focus_points: Vec<ZoomFocusPoint>,
    pub zoom_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomFocusPoint {
    pub time_ms: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdlePeriod {
    pub start_ms: f64,
    pub end_ms: f64,
    pub duration_ms: f64,
    pub classification: IdleClassification,
    pub suggested_speed: f64,
    pub preceding_action_type: String,
    pub following_action_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClickEffectKeyframe {
    pub video_timestamp_ms: f64,
    pub x: f64,
    pub y: f64,
    pub click_type: ClickType,
    pub action_index: usize,
    pub has_modifiers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeystrokeEvent {
    pub video_timestamp_ms: f64,
    pub display_text: String,
    pub event_type: KeystrokeEventType,
    pub display_duration_ms: f64,
    pub action_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPathKeyframe {
    pub video_timestamp_ms: f64,
    pub x: f64,
    pub y: f64,
    pub cursor_type: CursorType,
    pub velocity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorPath {
    pub style: MotionStyle,
    #[serde(default)]
    pub keyframes: Vec<CursorPathKeyframe>,
}

impl CursorPath {
    pub fn from_map(map: HashMap<MotionStyle, Vec<CursorPathKeyframe>>) -> Vec<Self> {
        map.into_iter()
            .map(|(style, keyframes)| CursorPath { style, keyframes })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderProxyArtifact {
    pub name: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub codec: String,
    pub profile: String,
    pub keyint: u32,
    pub status: String,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderProxiesMetadata {
    pub profile_version: String,
    pub generated_at_epoch_ms: u64,
    #[serde(default)]
    pub source: ProxySourceMetadata,
    #[serde(default)]
    pub artifacts: Vec<RenderProxyArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProxySourceMetadata {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    pub fps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecordingDataSummary {
    pub render_proxies: Option<RenderProxiesMetadata>,
}

fn deserialize_cursor_paths<'de, D>(deserializer: D) -> Result<Vec<CursorPath>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;

    if value.is_null() {
        return Ok(Vec::new());
    }

    if let Ok(list) = Vec::<CursorPath>::deserialize(&value) {
        return Ok(list);
    }

    let map = HashMap::<MotionStyle, Vec<CursorPathKeyframe>>::deserialize(&value)
        .map_err(D::Error::custom)?;

    Ok(CursorPath::from_map(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_cursor_paths_from_map() {
        let value = json!({
            "cursorPaths": {
                "2": [
                    { "videoTimestampMs": 100.0, "x": 0.1, "y": 0.2, "cursorType": 2, "velocity": 1.0 }
                ]
            }
        });

        let parsed: DecisionInput = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.cursor_paths.len(), 1);
        assert_eq!(parsed.cursor_paths[0].style, MotionStyle::Mellow);
        assert_eq!(parsed.cursor_paths[0].keyframes.len(), 1);
    }
}
