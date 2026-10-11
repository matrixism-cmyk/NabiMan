use chrono::{DateTime, Datelike, Timelike, Utc};
/// Numeric UTC cron: lists, inclusive ranges, steps, Sunday 0 or 7.
pub(super) struct Cron {
    fields: Vec<Vec<u32>>,
    day_star: bool,
    week_star: bool,
}
impl Cron {
    pub fn parse(input: &str) -> Result<Self, String> {
        let parts: Vec<_> = input.split_whitespace().collect();
        let invalid = || "Invalid UTC cron expression (five numeric fields required)".to_string();
        if parts.len() != 5 || input.len() > 120 {
            return Err(invalid());
        }
        let mut fields = Vec::new();
        for (part, (min, max)) in parts
            .iter()
            .zip([(0, 59), (0, 23), (1, 31), (1, 12), (0, 7)])
        {
            let mut values = Vec::new();
            for item in part.split(',') {
                let bits: Vec<_> = item.split('/').collect();
                if bits.len() > 2 {
                    return Err(invalid());
                }
                let number = |s: &str| -> Result<u32, String> {
                    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(invalid());
                    }
                    s.parse().map_err(|_| invalid())
                };
                let step = if bits.len() == 2 { number(bits[1])? } else { 1 };
                if step == 0 || step > max - min + 1 {
                    return Err(invalid());
                }
                let (start, end) = if bits[0] == "*" {
                    (min, max)
                } else if let Some((a, b)) = bits[0].split_once('-') {
                    (number(a)?, number(b)?)
                } else {
                    let n = number(bits[0])?;
                    if bits.len() == 2 {
                        return Err(invalid());
                    }
                    (n, n)
                };
                if start < min || end > max || start > end {
                    return Err(invalid());
                }
                values.extend((start..=end).step_by(step as usize));
            }
            fields.push(values);
        }
        Ok(Self {
            fields,
            day_star: parts[2].starts_with('*'),
            week_star: parts[4].starts_with('*'),
        })
    }
    pub fn matches(&self, now: DateTime<Utc>) -> bool {
        let day = self.fields[2].contains(&now.day());
        let week = now.weekday().num_days_from_sunday();
        let weekday = self.fields[4].contains(&week) || (week == 0 && self.fields[4].contains(&7));
        self.fields[0].contains(&now.minute())
            && self.fields[1].contains(&now.hour())
            && self.fields[3].contains(&now.month())
            && if self.day_star || self.week_star {
                day && weekday
            } else {
                day || weekday
            }
    }
}
