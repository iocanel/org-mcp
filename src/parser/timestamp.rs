use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

lazy_static! {
    // Active timestamp: <2026-03-06 Fri> or <2026-03-06 Fri 10:00>
    static ref ACTIVE_TS_RE: Regex = Regex::new(
        r"<(\d{4}-\d{2}-\d{2})\s+\w+(?:\s+(\d{2}:\d{2})(?:-(\d{2}:\d{2}))?)?(?:\s+([.+]+\d+[dwmy](?:/\d+[dwmy])?))?>"
    ).unwrap();

    // Inactive timestamp: [2026-03-06 Fri] or [2026-03-06 Fri 10:00]
    static ref INACTIVE_TS_RE: Regex = Regex::new(
        r"\[(\d{4}-\d{2}-\d{2})\s+\w+(?:\s+(\d{2}:\d{2})(?:-(\d{2}:\d{2}))?)?\]"
    ).unwrap();

    // SCHEDULED/DEADLINE prefix
    static ref SCHEDULED_RE: Regex = Regex::new(r"SCHEDULED:\s*(<[^>]+>)").unwrap();
    static ref DEADLINE_RE: Regex = Regex::new(r"DEADLINE:\s*(<[^>]+>)").unwrap();
    static ref CLOSED_RE: Regex = Regex::new(r"CLOSED:\s*(\[[^\]]+\])").unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimestampType {
    Active,
    Inactive,
    Scheduled,
    Deadline,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgTimestamp {
    pub timestamp_type: TimestampType,
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub repeater: Option<String>,
}

impl OrgTimestamp {
    pub fn parse_active(text: &str) -> Option<Self> {
        let caps = ACTIVE_TS_RE.captures(text)?;
        let date = NaiveDate::parse_from_str(caps.get(1)?.as_str(), "%Y-%m-%d").ok()?;
        let time = caps
            .get(2)
            .and_then(|m| NaiveTime::parse_from_str(m.as_str(), "%H:%M").ok());
        let end_time = caps
            .get(3)
            .and_then(|m| NaiveTime::parse_from_str(m.as_str(), "%H:%M").ok());
        let repeater = caps.get(4).map(|m| m.as_str().to_string());

        Some(Self {
            timestamp_type: TimestampType::Active,
            date,
            time,
            end_time,
            repeater,
        })
    }

    pub fn parse_inactive(text: &str) -> Option<Self> {
        let caps = INACTIVE_TS_RE.captures(text)?;
        let date = NaiveDate::parse_from_str(caps.get(1)?.as_str(), "%Y-%m-%d").ok()?;
        let time = caps
            .get(2)
            .and_then(|m| NaiveTime::parse_from_str(m.as_str(), "%H:%M").ok());
        let end_time = caps
            .get(3)
            .and_then(|m| NaiveTime::parse_from_str(m.as_str(), "%H:%M").ok());

        Some(Self {
            timestamp_type: TimestampType::Inactive,
            date,
            time,
            end_time,
            repeater: None,
        })
    }

    pub fn parse_scheduled(text: &str) -> Option<Self> {
        let caps = SCHEDULED_RE.captures(text)?;
        let ts_str = caps.get(1)?.as_str();
        let mut ts = Self::parse_active(ts_str)?;
        ts.timestamp_type = TimestampType::Scheduled;
        Some(ts)
    }

    pub fn parse_deadline(text: &str) -> Option<Self> {
        let caps = DEADLINE_RE.captures(text)?;
        let ts_str = caps.get(1)?.as_str();
        let mut ts = Self::parse_active(ts_str)?;
        ts.timestamp_type = TimestampType::Deadline;
        Some(ts)
    }

    pub fn parse_closed(text: &str) -> Option<Self> {
        let caps = CLOSED_RE.captures(text)?;
        let ts_str = caps.get(1)?.as_str();
        let mut ts = Self::parse_inactive(ts_str)?;
        ts.timestamp_type = TimestampType::Closed;
        Some(ts)
    }

    pub fn datetime(&self) -> NaiveDateTime {
        let time = self.time.unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        NaiveDateTime::new(self.date, time)
    }

    pub fn is_repeating(&self) -> bool {
        self.repeater.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_active_date_only() {
        let ts = OrgTimestamp::parse_active("<2026-03-06 Fri>").unwrap();
        assert_eq!(ts.timestamp_type, TimestampType::Active);
        assert_eq!(ts.date, NaiveDate::from_ymd_opt(2026, 3, 6).unwrap());
        assert!(ts.time.is_none());
        assert!(ts.end_time.is_none());
        assert!(ts.repeater.is_none());
    }

    #[test]
    fn test_parse_active_with_time() {
        let ts = OrgTimestamp::parse_active("<2026-03-06 Fri 10:00>").unwrap();
        assert_eq!(ts.date, NaiveDate::from_ymd_opt(2026, 3, 6).unwrap());
        assert_eq!(ts.time, Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap()));
        assert!(ts.end_time.is_none());
    }

    #[test]
    fn test_parse_active_with_time_range() {
        let ts = OrgTimestamp::parse_active("<2026-03-06 Fri 10:00-11:00>").unwrap();
        assert_eq!(ts.time, Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap()));
        assert_eq!(ts.end_time, Some(NaiveTime::from_hms_opt(11, 0, 0).unwrap()));
    }

    #[test]
    fn test_parse_active_with_repeater() {
        let ts = OrgTimestamp::parse_active("<2026-03-06 Fri .+1d>").unwrap();
        assert_eq!(ts.repeater, Some(".+1d".to_string()));
        assert!(ts.is_repeating());
    }

    #[test]
    fn test_parse_active_with_range_repeater() {
        let ts = OrgTimestamp::parse_active("<2026-03-06 Fri .+1d/3d>").unwrap();
        assert_eq!(ts.repeater, Some(".+1d/3d".to_string()));
    }

    #[test]
    fn test_parse_inactive() {
        let ts = OrgTimestamp::parse_inactive("[2026-03-05 Thu 10:00]").unwrap();
        assert_eq!(ts.timestamp_type, TimestampType::Inactive);
        assert_eq!(ts.date, NaiveDate::from_ymd_opt(2026, 3, 5).unwrap());
        assert_eq!(ts.time, Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap()));
    }

    #[test]
    fn test_parse_scheduled() {
        let ts = OrgTimestamp::parse_scheduled("SCHEDULED: <2026-03-06 Fri>").unwrap();
        assert_eq!(ts.timestamp_type, TimestampType::Scheduled);
        assert_eq!(ts.date, NaiveDate::from_ymd_opt(2026, 3, 6).unwrap());
    }

    #[test]
    fn test_parse_deadline() {
        let ts = OrgTimestamp::parse_deadline("DEADLINE: <2026-03-10 Tue>").unwrap();
        assert_eq!(ts.timestamp_type, TimestampType::Deadline);
        assert_eq!(ts.date, NaiveDate::from_ymd_opt(2026, 3, 10).unwrap());
    }

    #[test]
    fn test_parse_closed() {
        let ts = OrgTimestamp::parse_closed("CLOSED: [2026-03-05 Thu 10:00]").unwrap();
        assert_eq!(ts.timestamp_type, TimestampType::Closed);
        assert_eq!(ts.date, NaiveDate::from_ymd_opt(2026, 3, 5).unwrap());
    }

    #[test]
    fn test_datetime() {
        let ts = OrgTimestamp::parse_active("<2026-03-06 Fri 10:30>").unwrap();
        let dt = ts.datetime();
        assert_eq!(dt.date(), NaiveDate::from_ymd_opt(2026, 3, 6).unwrap());
        assert_eq!(dt.time(), NaiveTime::from_hms_opt(10, 30, 0).unwrap());
    }

    #[test]
    fn test_datetime_no_time() {
        let ts = OrgTimestamp::parse_active("<2026-03-06 Fri>").unwrap();
        let dt = ts.datetime();
        assert_eq!(dt.time(), NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    }

    #[test]
    fn test_parse_invalid() {
        assert!(OrgTimestamp::parse_active("invalid").is_none());
        assert!(OrgTimestamp::parse_inactive("invalid").is_none());
        assert!(OrgTimestamp::parse_scheduled("invalid").is_none());
        assert!(OrgTimestamp::parse_deadline("invalid").is_none());
        assert!(OrgTimestamp::parse_closed("invalid").is_none());
    }
}
