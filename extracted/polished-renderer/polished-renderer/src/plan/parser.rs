use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

use super::types::{RecordingDataSummary, RenderPlan};

pub fn load_plan(path: &Path) -> Result<RenderPlan> {
    let contents = fs::read_to_string(path)?;
    let plan: RenderPlan = serde_json::from_str(&contents)?;
    Ok(plan)
}

pub fn default_plan_path(session_dir: &Path) -> PathBuf {
    session_dir.join("recording").join("render-plan.json")
}

pub fn load_recording_data(session_dir: &Path) -> Result<Option<RecordingDataSummary>> {
    let path = session_dir.join("recording").join("recording-data.json");
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)?;
    let parsed: RecordingDataSummary = serde_json::from_str(&contents)?;
    Ok(Some(parsed))
}
