use std::{fmt::Display, iter::once, str::FromStr};

use anyhow::{bail, Result};
use itertools::Itertools;
use ont::Outline;

use crate::IoPipe;

#[derive(Clone, Default)]
struct Cell {
    text: String,
    value: Option<f64>,
    formula: Option<String>,
}

impl FromStr for Cell {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        let text = s.to_string();

        let mut formula = None;
        let mut value = None;

        if let Some((val, form)) = s.split_once(':') {
            if let Ok(val) = val.parse::<f64>() {
                value = Some(val);
                formula = Some(form.to_string());
            } else if val.is_empty() {
                value = Some(0.0);
                formula = Some(form.to_string());
            } else {
                // It's not a valid formula cell...
                // Treat it as text cell.
            }
        } else if let Ok(val) = s.parse::<f64>() {
            // No formula, but it's a valid number.
            value = Some(val);
        } else if s == "-" {
            // Missing value indicator, treating these as zeroes for now.
            value = Some(0.0);
        }

        Ok(Cell {
            text,
            value,
            formula,
        })
    }
}

impl Cell {
    /// Constructor that forces a text cell.
    fn text(text: impl Into<String>) -> Self {
        Cell {
            text: text.into(),
            value: None,
            formula: None,
        }
    }

    fn is_numeric(&self) -> bool {
        self.value.is_some()
    }

    fn len(&self) -> usize {
        self.text.len()
    }

    fn value_str(&self) -> &str {
        match (self.value, &self.formula) {
            (Some(_), Some(f)) => &self.text[0..self.text.len() - f.len() - 1],
            (Some(_), None) => &self.text,
            (None, _) => "",
        }
    }

    /// Find how much the cell must be shifted to the left so it'll align with
    /// other numbers at the exponent marker or the decimal point.
    fn left_extension(&self) -> usize {
        let s = self.value_str();

        if let Some(pos) = s.find('e') {
            pos // Try to align by exponent marker first,
        } else if let Some(pos) = s.find('.') {
            pos // then by the decimal point,
        } else {
            // Otherwise use the whole number string length. If this is a text
            // cell, value_str will be empty and we get 0 here, which is
            // correct because text cells are left-aligned.
            s.len()
        }
    }

    fn assign(&mut self, value: f64) {
        self.value = Some(value);
        let s = format_float(value);
        if let Some(formula) = &self.formula {
            self.text = format!("{s}:{formula}");
        } else {
            // Would we ever be assigning to a non-formula cell?
            self.text = s;
        }
    }
}

impl Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

#[derive(Default)]
struct Table {
    columns: usize,
    cells: Vec<Vec<Cell>>,
}

impl Table {
    /// Construct a new table from an outline with parsed cells and column
    /// count.
    fn new(
        outline: &Outline,
        num_columns: usize,
        parse_numbers: bool,
    ) -> Result<Self> {
        let mut table = Table {
            columns: if num_columns == 0 {
                usize::MAX
            } else {
                num_columns
            },
            cells: Vec::new(),
        };

        // Figure out how many columns we'll be aligning.
        //
        // IDM tables can have trailing data after the last column which varies
        // per row. This will not be aligned. So we're looking for the minimum
        // number of whitespace-separated words every non-empty line has.
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
            table.columns = table.columns.min(line_columns);
        }

        if is_empty {
            // No content seen, return the empty table.
            return Ok(table);
        }

        for s in outline.iter() {
            let head = s.head.trim();
            if head.is_empty() {
                table.cells.push(Vec::new());
                continue;
            }

            // Offsets where words start in the line.
            //
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
                if i < table.columns {
                    // Keep pushing individual words while we have columns.
                    let cell = if parse_numbers {
                        head[a..b].parse()?
                    } else {
                        // If number parsing is disabled, all cells are forced
                        // to be text.
                        Cell::text(head[a..b].trim())
                    };
                    row.push(cell);
                } else {
                    // If there's still content left, push all of it to the extra
                    // column that's never numeric.
                    row.push(Cell::text(head[a..].trim()));
                    break;
                }
            }

            // Sanity-check our earlier column calculation.
            assert!((table.columns..=(table.columns + 1)).contains(&row.len()));
            table.cells.push(row);
        }

        Ok(table)
    }

    /// Return numeric cell values left of target as stack.
    fn stack(&self, row: usize, col: usize) -> Vec<f64> {
        if self.cells.len() < row {
            return Vec::new();
        }
        // Add only numeric cells to stack.
        let ret: Vec<f64> = self.cells[row]
            .iter()
            .take(col)
            .filter_map(|c| c.value)
            .collect();
        ret
    }

    /// Return numeric cell values above target as stack.
    fn vertical_stack(&self, row: usize, column: usize) -> Vec<f64> {
        let mut ret = Vec::new();
        for r in 0..row {
            if let Some(c) = self.cells.get(r).and_then(|row| row.get(column)) {
                if let Some(v) = c.value {
                    ret.push(v);
                }
            }
        }
        ret
    }

    /// Evalaute spreadsheet formulas in all cells and insert their results.
    fn eval(&mut self) -> Result<()> {
        for row in 0..self.cells.len() {
            for col in 0..self.cells[row].len() {
                self.eval_cell(row, col)?;
            }
        }
        Ok(())
    }

    fn eval_cell(&mut self, row: usize, col: usize) -> Result<()> {
        let Some(c) = self
            .cells
            .get_mut(row)
            .and_then(|row| row.get_mut(col))
            .cloned()
        else {
            return Ok(());
        };

        let Some(formula) = &c.formula else {
            return Ok(());
        };

        let mut s = self.stack(row, col);
        let mut stack_length = s.len();

        // Flag that we switched to a vertical stack.
        let mut is_vertical = false;

        // Number literal accumulator.
        let mut acc = f64::NAN;

        for c in formula.chars() {
            // Number literal parsing, only natural numbers are supported.
            if c.is_ascii_digit() {
                if acc.is_nan() {
                    acc = 0.0;
                }

                // Accumulate digits into a number.
                acc = acc * 10.0 + (c as u8 - b'0') as f64;
                continue;
            } else if !acc.is_nan() {
                s.push(acc);
                acc = f64::NAN;
            }

            // Weird glyphs are inspired by uiua.
            match c {
                '+' => {
                    // Addition
                    let val = pop(&mut s)? + pop(&mut s)?;
                    s.push(val);
                }
                '-' => {
                    // Subtraction
                    let a = pop(&mut s)?;
                    let b = pop(&mut s)?;
                    s.push(b - a);
                }
                '%' | '÷' => {
                    // Division
                    let a = pop(&mut s)?;
                    let b = pop(&mut s)?;
                    s.push(b / a);
                }
                'x' => {
                    // Multiplication
                    let val = pop(&mut s)? * pop(&mut s)?;
                    s.push(val);
                }
                '·' => {
                    // Drop stack item
                    pop(&mut s)?;
                }
                '⁅' => {
                    // Round to nearest integer.
                    let val = pop(&mut s)?;
                    s.push(val.round());
                }
                '#' => {
                    // Initial stack length.
                    // We generally never want the length of the stack after
                    // we've started operating on it, so this returns a cached
                    // value from when the stack was initialized.
                    s.push(stack_length as f64);
                }
                '√' => {
                    // Square root
                    let val = pop(&mut s)?;
                    s.push(val.sqrt());
                }
                // If we had reduce (/), sum and product would be shorthand
                // for /+ and /*
                'Σ' | 'S' => {
                    // Stack sum
                    let sum: f64 = s.iter().sum();
                    s.clear();
                    s.push(sum);
                }
                'Π' => {
                    // Stack product
                    let prod: f64 = s.iter().product();
                    s.clear();
                    s.push(prod);
                }
                'v' | '↓' => {
                    // Switch to vertical stack (only works once)
                    if !is_vertical {
                        is_vertical = true;
                        s = self.vertical_stack(row, col);
                        stack_length = s.len();
                    }
                }

                c => {
                    bail!("tf: Unsupported formula character '{c}' at ({row}, {col})")
                }
            }
        }
        if let Some(result) = s.pop() {
            self.cells[row][col].assign(result);
        } else {
            bail!("tf: Stack underflow at ({row}, {col})")
        }
        Ok(())
    }
}

impl Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // (left-padding, right-padding) needed by each column.
        let mut column_extents = Vec::new();

        // Figure out column widths.

        // The optional final +1 column is never numeric and doesn't need any
        // padding, so the iteration skips it.
        for i in 0..self.columns {
            let (mut left_extent, mut right_extent) = (0, 0);
            for row in &self.cells {
                let Some(c) = row.get(i) else { continue };
                let left_extension = c.left_extension();

                left_extent = left_extent.max(left_extension);
            }

            // Non-numeric values will be left-alinged at the maximum left
            // extension.
            for row in &self.cells {
                let Some(c) = row.get(i) else { continue };
                let right_extension = if c.is_numeric() {
                    let left_extension = c.left_extension();
                    c.len() - left_extension
                } else {
                    c.len() - left_extent
                };

                right_extent = right_extent.max(right_extension);
            }
            column_extents.push((left_extent, right_extent));
        }

        // Print the table.
        for row in &self.cells {
            if row.is_empty() {
                writeln!(f)?;
                continue;
            }

            for (i, c) in row.iter().enumerate() {
                // The final bit, always push it in as is.
                if i >= self.columns {
                    write!(f, "{c}")?;
                    continue;
                }

                let (left_pos, right_pos) = column_extents[i];
                let left_extent = if c.is_numeric() {
                    c.left_extension()
                } else {
                    left_pos
                };

                let right_extent = c.len() - left_extent;

                // Pad to meet left pos. To stay IDM-compatible, the leftmost
                // column needs to be padded with NBSPs (\u{00A0}) that don't read as
                // whitespace to IDM.
                if left_extent < left_pos {
                    if i == 0 {
                        write!(
                            f,
                            "{:\u{00A0}^width$}",
                            "",
                            width = left_pos - left_extent
                        )?;
                    } else {
                        // Otherwise use spaces.
                        write!(
                            f,
                            "{: ^width$}",
                            "",
                            width = left_pos - left_extent
                        )?;
                    }
                }

                write!(f, "{c}")?;

                // Right-padding and space between columns.
                if i < row.len() - 1 {
                    write!(
                        f,
                        "{: <width$}",
                        "",
                        width = right_pos - right_extent + 2
                    )?;
                }
            }

            writeln!(f)?;
        }

        Ok(())
    }
}

pub fn run(
    no_number_parsing: bool,
    num_columns: usize,
    io: IoPipe,
) -> Result<()> {
    let outline = io.read_outline()?;

    let mut table = Table::new(&outline, num_columns, !no_number_parsing)?;
    table.eval()?;
    io.write_text(table.to_string())
}

fn pop(stack: &mut Vec<f64>) -> Result<f64> {
    stack
        .pop()
        .ok_or_else(|| anyhow::anyhow!("Stack underflow"))
}

fn format_float(x: f64) -> String {
    let mut s = x.to_string();

    // If we ended up with lots of decimals, truncate to 3, YAGNI more.

    let d = s.split('e').next().unwrap_or(&s); // Trim out exponent.
    let d = d.split('.').nth(1).unwrap_or(""); // Get decimal part.

    if d.len() > 3 {
        s = format!("{:.3}", x);
        // Remove trailing zeroes
        while s.ends_with('0') {
            s.pop();
        }
    }

    s
}
