use chrono::NaiveDateTime;

pub fn timestamp_to_datetime(timestamp: i64) -> String {
    NaiveDateTime::from_timestamp_opt(timestamp, 0)
        .map(|datetime| datetime.to_string())
        .unwrap_or_default()
}
