//! Sorting logic for DataTable.
//!
//! This module contains value comparison functions for different column types.

use std::cmp::Ordering;

use super::types::ColumnType;

/// Compare two optional string values based on column type.
pub(crate) fn compare_values(
    a: Option<&str>,
    b: Option<&str>,
    column_type: ColumnType,
) -> Ordering {
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => match column_type {
            ColumnType::Text => a.cmp(b),

            ColumnType::Integer => {
                let parsed_a = a.trim().replace(",", "").parse::<i64>().ok();
                let parsed_b = b.trim().replace(",", "").parse::<i64>().ok();
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::Float => {
                let parsed_a = a.trim().replace(",", "").parse::<f64>().ok();
                let parsed_b = b.trim().replace(",", "").parse::<f64>().ok();
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::Percent => {
                // Parse "45%" or "0.45" style percentages
                let parse_percent = |s: &str| -> Option<f64> {
                    let s = s.trim();
                    if let Some(stripped) = s.strip_suffix('%') {
                        stripped.trim().parse::<f64>().ok()
                    } else {
                        s.parse::<f64>().ok().map(|v| v * 100.0)
                    }
                };
                let parsed_a = parse_percent(a);
                let parsed_b = parse_percent(b);
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::Size => {
                // Parse "1.5 GB", "500 KB", etc.
                let parse_size = |s: &str| -> Option<u64> {
                    let s = s.trim().to_uppercase();
                    let multipliers = [
                        ("TB", 1024u64.pow(4)),
                        ("GB", 1024u64.pow(3)),
                        ("MB", 1024u64.pow(2)),
                        ("KB", 1024u64),
                        ("B", 1u64),
                    ];
                    for (suffix, mult) in multipliers {
                        if s.ends_with(suffix) {
                            let num_str = s[..s.len() - suffix.len()].trim();
                            if let Ok(num) = num_str.parse::<f64>() {
                                return Some((num * mult as f64) as u64);
                            }
                        }
                    }
                    // Try parsing as raw bytes
                    s.parse::<u64>().ok()
                };
                let parsed_a = parse_size(a);
                let parsed_b = parse_size(b);
                match (parsed_a, parsed_b) {
                    (Some(a), Some(b)) => a.cmp(&b),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.cmp(b),
                }
            }

            ColumnType::DateTime | ColumnType::Duration => {
                // Fall back to lexicographic comparison
                // ISO 8601 dates sort correctly lexicographically
                a.cmp(b)
            }
        },
    }
}
