use std::{fmt::Write, iter::once};

use anyhow::Result;
use idm_tools::Outline;
use itertools::Itertools;

use crate::IoPipe;

pub fn run(
    no_number_parsing: bool,
    num_columns: usize,
    io: IoPipe,
) -> Result<()> {
    fn is_numeric(s: &str) -> bool {
        let s = s.trim();

        // Conventional missing data indicator in IDM, allowed in numeric
        // columns.
        if s == "-" {
            return true;
        }

        s.parse::<f64>().is_ok()
    }

    /// Find how much a number must be shifted to the left so it'll align with
    /// other numbers at the exponent marker or the decimal point.
    fn number_left_extension(num: &str) -> usize {
        if let Some(e) = num.find('e') {
            e // Try to align by exponent marker first,
        } else if let Some(e) = num.find('.') {
            e // then by the decimal point,
        } else {
            num.len() // otherwise just right-align the whole thing.
        }
    }

    let outline = io.read_outline()?;

    let mut columns = if num_columns == 0 {
        usize::MAX
    } else {
        num_columns
    };

    // Figure out how many columns we'll be aligning.
    //
    // IDM tables can have trailing data after the last column which varies
    // per row. This will not be aligned.
    let mut is_empty = true;
    for s in outline.iter() {
        // Table must consist of only lines, if there's a section with a body,
        // bail out now.
        if !s.body.is_empty() {
            anyhow::bail!("tf: Input is not a table");
        }

        // Completely empty lines are allowed.
        if s.head.trim().is_empty() {
            continue;
        }

        is_empty = false;

        // Count whitespace-separated words in s.head
        let line_columns = s.head.split_whitespace().count();
        assert!(line_columns > 0);
        columns = columns.min(line_columns);
    }

    if is_empty {
        // No content in table, don't do anything.
        return io.write(&outline);
    }

    // Construct a table structure.
    let mut table: Vec<Vec<&str>> = Vec::new();
    for s in outline.iter() {
        let head = s.head.trim();
        if head.is_empty() {
            table.push(Vec::new());
            continue;
        }

        // Offsets where words start in the line.
        // Not using split_whitespace here because we want to preserve
        // whatever spacing the final trailing sections of the table lines
        // use between their words.
        let word_starts: Vec<usize> = head
            .char_indices()
            .zip(once(' ').chain(head.chars()))
            .filter_map(|((i, d), c)| {
                (c.is_whitespace() && !d.is_whitespace()).then_some(i)
            })
            .collect();

        let mut row = Vec::new();

        for (i, (&a, &b)) in word_starts
            .iter()
            .chain(Some(&head.len()))
            .tuple_windows()
            .enumerate()
        {
            if i < columns {
                // Keep pushing individual words while we have columns.
                row.push(head[a..b].trim());
            } else {
                // If there's still content left, push all of it to the extra
                // column.
                row.push(head[a..].trim());
                break;
            }
        }

        assert!((columns..=(columns + 1)).contains(&row.len()));
        table.push(row);
    }

    // Which columns are numeric.
    let mut numeric_columns = vec![false; columns + 1];

    // (left-padding, right-padding) needed by each column.
    let mut column_extents = Vec::new();

    // The optional final +1 column is never numeric and doesn't need any
    // padding, so the iteration skips it.
    for i in 0..columns {
        if no_number_parsing {
            break;
        }

        numeric_columns[i] = true;
        for row in &table {
            let Some(c) = row.get(i) else { continue };
            if !is_numeric(c) {
                numeric_columns[i] = false;
                break;
            }
        }

        let (mut left_extent, mut right_extent) = (0, 0);
        for row in &table {
            let Some(c) = row.get(i) else { continue };
            if numeric_columns[i] {
                let left_extension = number_left_extension(c);

                left_extent = left_extent.max(left_extension);
                right_extent = right_extent.max(c.len() - left_extension);
            } else {
                right_extent = right_extent.max(c.len());
            }
        }

        column_extents.push((left_extent, right_extent));
    }

    let mut output = Outline::default();
    for row in table {
        if row.is_empty() {
            output.push(Default::default());
            continue;
        }

        let mut line = String::new();

        for (i, c) in row.iter().enumerate() {
            // The final bit, always push it in as is.
            if i >= columns {
                write!(line, "{c}")?;
                continue;
            }

            let (left_pos, right_pos) = column_extents[i];
            let left_extent = if numeric_columns[i] {
                number_left_extension(c)
            } else {
                0
            };
            let right_extent = c.len() - left_extent;

            if left_extent < left_pos {
                if i == 0 {
                    // IDM does not like an uneven left edge, but only sees
                    // ASCII whitespace, so use NBSP as false whitespace for
                    // left-padding the leftmost column.
                    write!(
                        line,
                        "{:\u{00A0}^width$}",
                        "",
                        width = left_pos - left_extent
                    )?;
                } else {
                    // Otherwise use spaces.
                    write!(
                        line,
                        "{: ^width$}",
                        "",
                        width = left_pos - left_extent
                    )?;
                }
            }

            write!(line, "{c}")?;

            // Right-padding and space between columns.
            write!(
                line,
                "{: <width$}",
                "",
                width = right_pos - right_extent + 2
            )?;
        }

        output.push_line(line.trim_end());
    }

    io.write(&output)
}
