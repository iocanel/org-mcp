use crate::emacs::EmacsClient;
use crate::roam::{DrillStats, OrgRoamDatabase};
use anyhow::Result;
use chrono::Local;

/// Raw org-drill card counts from the roam database. This is the reliable
/// underlying signal, not the exact set a session will present (org-drill
/// applies new-card caps and its spaced-repetition schedule at drill time).
pub fn drill_status() -> Result<DrillStats> {
    let path = OrgRoamDatabase::find_database()?;
    let db = OrgRoamDatabase::open(&path)?;
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    db.drill_stats(&today)
}

/// Launch an interactive org-drill session in a new Emacs frame. Non-blocking:
/// org-drill runs a review loop, so it must NOT be evaluated in the daemon
/// (that would hang). Opens a client frame and returns immediately.
pub async fn start_drill(emacs: &EmacsClient) -> Result<()> {
    emacs.spawn_frame("(org-drill)").await
}
