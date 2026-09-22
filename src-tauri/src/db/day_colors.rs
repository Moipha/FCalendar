use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DayColorPreset {
    Green,
    Red,
}

impl DayColorPreset {
    fn as_str(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Red => "red",
        }
    }

    fn from_db(s: &str) -> Option<Self> {
        match s {
            "green" => Some(Self::Green),
            "red" => Some(Self::Red),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayColorRow {
    pub date: String,
    pub color: String,
}

pub fn list_day_colors(
    conn: &Connection,
    from: &str,
    to: &str,
) -> Result<Vec<DayColorRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT date, color FROM day_colors
             WHERE date >= ?1 AND date <= ?2
             ORDER BY date ASC",
        )
        .map_err(|e| format!("prepare list_day_colors: {e}"))?;

    let rows = stmt
        .query_map(params![from, to], |row| {
            let date: String = row.get(0)?;
            let color_str: String = row.get(1)?;
            Ok((date, color_str))
        })
        .map_err(|e| format!("query list_day_colors: {e}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("collect list_day_colors: {e}"))?;

    let mut out = Vec::with_capacity(rows.len());
    for (date, color_str) in rows {
        if DayColorPreset::from_db(&color_str).is_none() {
            continue;
        }
        out.push(DayColorRow {
            date,
            color: color_str,
        });
    }
    Ok(out)
}

/// `color` 为 `None` 或空字符串时删除该日记录（恢复默认底色）。
pub fn set_day_color(
    conn: &Connection,
    date: &str,
    color: Option<&str>,
) -> Result<(), String> {
    if date.len() != 10 || !date.as_bytes().get(4).is_some_and(|b| *b == b'-') {
        return Err("invalid date".into());
    }

    match color.filter(|s| !s.is_empty()) {
        None => {
            conn.execute("DELETE FROM day_colors WHERE date = ?1", params![date])
                .map_err(|e| format!("delete day_color: {e}"))?;
        }
        Some("green") => {
            conn.execute(
                "INSERT INTO day_colors (date, color) VALUES (?1, 'green')
                 ON CONFLICT(date) DO UPDATE SET color = 'green'",
                params![date],
            )
            .map_err(|e| format!("set day_color green: {e}"))?;
        }
        Some("red") => {
            conn.execute(
                "INSERT INTO day_colors (date, color) VALUES (?1, 'red')
                 ON CONFLICT(date) DO UPDATE SET color = 'red'",
                params![date],
            )
            .map_err(|e| format!("set day_color red: {e}"))?;
        }
        Some(_) => return Err("invalid color".into()),
    }
    Ok(())
}

pub fn set_day_colors(
    conn: &Connection,
    dates: &[String],
    color: Option<&str>,
) -> Result<(), String> {
    for date in dates {
        set_day_color(conn, date, color)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrate;

    fn test_conn() -> Connection {
        let dir = std::env::temp_dir().join(format!("fcalendar-day-colors-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.db");
        let _ = std::fs::remove_file(&path);
        migrate(&path).unwrap();
        Connection::open(&path).unwrap()
    }

    #[test]
    fn set_list_and_clear_day_color() {
        let conn = test_conn();
        assert!(list_day_colors(&conn, "2026-01-01", "2026-12-31")
            .unwrap()
            .is_empty());

        set_day_color(&conn, "2026-03-15", Some("green")).unwrap();
        let rows = list_day_colors(&conn, "2026-03-01", "2026-03-31").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-03-15");
        assert_eq!(rows[0].color, "green");

        set_day_color(&conn, "2026-03-15", Some("red")).unwrap();
        let rows = list_day_colors(&conn, "2026-03-01", "2026-03-31").unwrap();
        assert_eq!(rows[0].color, "red");

        set_day_color(&conn, "2026-03-15", None).unwrap();
        assert!(list_day_colors(&conn, "2026-03-01", "2026-03-31")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn set_day_colors_batch() {
        let conn = test_conn();
        set_day_colors(
            &conn,
            &["2026-04-01".into(), "2026-04-02".into()],
            Some("green"),
        )
        .unwrap();
        let rows = list_day_colors(&conn, "2026-04-01", "2026-04-30").unwrap();
        assert_eq!(rows.len(), 2);
        set_day_colors(&conn, &["2026-04-01".into(), "2026-04-02".into()], None).unwrap();
        assert!(list_day_colors(&conn, "2026-04-01", "2026-04-30")
            .unwrap()
            .is_empty());
    }
}
