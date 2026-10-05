//! Decides from every value of a text column whether it holds dates and in
//! which order (RV18). A column only counts as dates when all its values parse
//! with the same order, so typing it never loses a value; when both day/month
//! and month/day fit every value, the order is ambiguous and the person picks.

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DateOrder {
    /// Year first: 2024-02-01, 2024/02/01, ISO 8601.
    Iso,
    /// Day first: 01/02/2024, 1-2-2024.
    Dmy,
    /// Month first: 02/01/2024, 12/1/2010 8:26.
    Mdy,
}

impl DateOrder {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Dmy => "dmy",
            Self::Mdy => "mdy",
        }
    }

    pub(super) fn from_label(label: &str) -> Option<Self> {
        match label {
            "iso" => Some(Self::Iso),
            "dmy" => Some(Self::Dmy),
            "mdy" => Some(Self::Mdy),
            _ => None,
        }
    }
}

const ORDERS: [DateOrder; 3] = [DateOrder::Iso, DateOrder::Dmy, DateOrder::Mdy];

fn parse_time(text: &str) -> Option<NaiveTime> {
    let text = text.trim_end_matches('Z');
    NaiveTime::parse_from_str(text, "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M"))
        .ok()
}

/// Separators a date may use between its three parts.
pub(super) const DATE_SEPARATORS: &[char] = &['/', '-', '.'];

/// Parses a date without time in one order, the only date parser behind
/// Preparar, the import conventions and the recipes (FUN-02). The year has
/// four digits; with `two_digit_years`, day- or month-first dates also accept
/// two, following Excel's rule (00-29 is 20xx, 30-99 is 19xx). Years of one
/// or three digits are never a date, so `01/02/25` cannot become year 0025.
pub(super) fn parse_calendar_date(
    date_text: &str,
    order: DateOrder,
    separators: &[char],
    two_digit_years: bool,
) -> Option<NaiveDate> {
    let separator = date_text
        .chars()
        .find(|character| DATE_SEPARATORS.contains(character))?;
    if !separators.contains(&separator) {
        return None;
    }
    let parts = date_text.split(separator).collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    let (day_text, month_text, year_text) = match order {
        DateOrder::Iso => (parts[2], parts[1], parts[0]),
        DateOrder::Dmy => (parts[0], parts[1], parts[2]),
        DateOrder::Mdy => (parts[1], parts[0], parts[2]),
    };
    if day_text.len() > 2 || month_text.len() > 2 {
        return None;
    }
    let year = year_text.parse::<i32>().ok()?;
    let year = match (order, year_text.len()) {
        (_, 4) => year,
        (DateOrder::Dmy | DateOrder::Mdy, 2) if two_digit_years && year < 30 => 2000 + year,
        (DateOrder::Dmy | DateOrder::Mdy, 2) if two_digit_years => 1900 + year,
        _ => return None,
    };
    NaiveDate::from_ymd_opt(year, month_text.parse().ok()?, day_text.parse().ok()?)
}

/// Parses one value with one order; `None` when it is not a date in it.
pub(super) fn parse_ordered_date(value: &str, order: DateOrder) -> Option<NaiveDateTime> {
    let value = value.trim();
    let (date_text, time_text) = match value.split_once(['T', ' ']) {
        Some((date, time)) => (date, Some(time.trim())),
        None => (value, None),
    };
    // Two-digit years follow Excel's rule: it is how Excel wrote them, and
    // the proposal shows the result first.
    let date = parse_calendar_date(date_text, order, DATE_SEPARATORS, true)?;
    if !super::PLAUSIBLE_DATE_YEARS.contains(&date.year()) {
        return None;
    }
    let time = match time_text {
        Some(text) => parse_time(text)?,
        None => NaiveTime::MIN,
    };
    Some(date.and_time(time))
}

/// What a whole column says about its dates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DateInference {
    Single {
        order: DateOrder,
        has_time: bool,
    },
    /// Day/month and month/day both fit every value.
    Ambiguous {
        has_time: bool,
    },
}

impl DateInference {
    pub(super) fn order_label(self) -> &'static str {
        match self {
            Self::Single { order, .. } => order.label(),
            Self::Ambiguous { .. } => "ambiguous",
        }
    }

    pub(super) fn has_time(self) -> bool {
        match self {
            Self::Single { has_time, .. } | Self::Ambiguous { has_time } => has_time,
        }
    }
}

/// Accumulates values one at a time, so the profile can feed it from the loop
/// it already runs; it stops trying once no order fits.
#[derive(Debug)]
pub(super) struct DateTally {
    fits: [bool; 3],
    has_time: bool,
    values: usize,
}

impl Default for DateTally {
    fn default() -> Self {
        Self {
            fits: [true; 3],
            has_time: false,
            values: 0,
        }
    }
}

impl DateTally {
    pub(super) fn observe(&mut self, value: &str) {
        if !self.fits.contains(&true) {
            return;
        }
        let value = value.trim();
        self.values += 1;
        for (fits, order) in self.fits.iter_mut().zip(ORDERS) {
            if *fits && parse_ordered_date(value, order).is_none() {
                *fits = false;
            }
        }
        if value.contains(':') {
            self.has_time = true;
        }
    }

    pub(super) fn finish(&self) -> Option<DateInference> {
        if self.values == 0 {
            return None;
        }
        let has_time = self.has_time;
        match self.fits {
            [true, _, _] => Some(DateInference::Single {
                order: DateOrder::Iso,
                has_time,
            }),
            [false, true, true] => Some(DateInference::Ambiguous { has_time }),
            [false, true, false] => Some(DateInference::Single {
                order: DateOrder::Dmy,
                has_time,
            }),
            [false, false, true] => Some(DateInference::Single {
                order: DateOrder::Mdy,
                has_time,
            }),
            [false, false, false] => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn infer(values: &[&str]) -> Option<DateInference> {
        let mut tally = DateTally::default();
        for value in values {
            tally.observe(value);
        }
        tally.finish()
    }

    #[test]
    fn a_day_above_twelve_decides_the_order_for_the_whole_column() {
        // OnlineRetail: month first, with time.
        assert_eq!(
            infer(&["12/1/2010 8:26", "12/13/2010 9:01", "1/2/2011 10:00"]),
            Some(DateInference::Single {
                order: DateOrder::Mdy,
                has_time: true
            })
        );
        assert_eq!(
            infer(&["01/02/2024", "25/12/2024", "3-4-2024"]),
            Some(DateInference::Single {
                order: DateOrder::Dmy,
                has_time: false
            })
        );
        assert_eq!(
            infer(&["2024-02-01", "2024-12-25T10:30:00"]),
            Some(DateInference::Single {
                order: DateOrder::Iso,
                has_time: true
            })
        );
    }

    #[test]
    fn only_ambiguous_values_leave_the_order_to_the_person() {
        assert_eq!(
            infer(&["01/02/2024", "03/04/2024"]),
            Some(DateInference::Ambiguous { has_time: false })
        );
    }

    #[test]
    fn one_value_that_is_not_a_date_keeps_the_column_as_text() {
        assert_eq!(infer(&["01/02/2024", "pendiente"]), None);
        assert_eq!(infer(&["2024"]), None, "a year alone is not a date");
        assert_eq!(infer(&["12345"]), None);
        assert_eq!(infer(&["31/02/2024"]), None, "no February 31st");
        assert_eq!(infer(&[]), None);
    }

    #[test]
    fn two_digit_years_follow_the_excel_rule() {
        // OnlineRetail, exported by Excel in English: m/d/yy h:mm.
        assert_eq!(
            infer(&["12/1/10 8:26", "12/13/10 9:01"]),
            Some(DateInference::Single {
                order: DateOrder::Mdy,
                has_time: true
            })
        );
        let parsed = parse_ordered_date("12/1/10 8:26", DateOrder::Mdy).unwrap();
        assert_eq!(parsed.to_string(), "2010-12-01 08:26:00");
        let parsed = parse_ordered_date("1/2/45", DateOrder::Dmy).unwrap();
        assert_eq!(parsed.to_string(), "1945-02-01 00:00:00");
        assert_eq!(
            parse_ordered_date("24-01-02", DateOrder::Iso),
            None,
            "year first needs four digits"
        );
    }

    #[test]
    fn parses_the_value_with_the_chosen_order() {
        let parsed = parse_ordered_date("12/1/2010 8:26", DateOrder::Mdy).unwrap();
        assert_eq!(parsed.to_string(), "2010-12-01 08:26:00");
        let parsed = parse_ordered_date("12/1/2010", DateOrder::Dmy).unwrap();
        assert_eq!(parsed.to_string(), "2010-01-12 00:00:00");
    }
}
