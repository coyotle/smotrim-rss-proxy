use chrono::format::strftime::StrftimeItems;
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Europe;
use std::collections::HashMap;

pub fn format_rfc822(datetime: DateTime<Utc>) -> String {
    let format = StrftimeItems::new("%a, %d %b %Y %H:%M:%S %z");
    datetime.format_with_items(format).to_string()
}

pub fn parse_custom_date(date_str: &str) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
    let date_str = date_str.trim_matches('"');

    if date_str.len() <= 5 && date_str.contains(':') {
        parse_time_only(date_str)
    } else {
        parse_full_date(date_str)
    }
}

fn parse_time_only(time_str: &str) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
    let today = Local::now().date_naive();
    let time = NaiveTime::parse_from_str(time_str, "%H:%M")?;
    let datetime = NaiveDateTime::new(today, time);
    let moscow_time = Europe::Moscow
        .from_local_datetime(&datetime)
        .single()
        .unwrap();

    Ok(moscow_time.with_timezone(&Utc))
}

fn parse_full_date(date_str: &str) -> Result<DateTime<Utc>, Box<dyn std::error::Error>> {
    let months_ru = [
        ("января", 1),
        ("февраля", 2),
        ("марта", 3),
        ("апреля", 4),
        ("мая", 5),
        ("июня", 6),
        ("июля", 7),
        ("августа", 8),
        ("сентября", 9),
        ("октября", 10),
        ("ноября", 11),
        ("декабря", 12),
    ]
    .iter()
    .cloned()
    .collect::<HashMap<&str, u32>>();

    let parts: Vec<&str> = date_str.split_whitespace().collect();
    if parts.len() != 3 {
        return Err("Invalid date format".into());
    }

    let day = parts[0].parse::<u32>()?;
    let month = *months_ru.get(parts[1]).ok_or("Invalid month")?;
    let year = parts[2].parse::<i32>()?;

    let date = NaiveDate::from_ymd_opt(year, month, day).ok_or("Invalid date")?;
    let datetime = NaiveDateTime::new(date, NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    let moscow_time = Europe::Moscow
        .from_local_datetime(&datetime)
        .single()
        .unwrap();

    Ok(moscow_time.with_timezone(&Utc))
}
