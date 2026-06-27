use std::{
    fmt::Display,
    iter::once,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use anyhow::{bail, Result};
use itertools::Itertools;
use ont::Outline;

use crate::IoPipe;

pub fn run(
    no_number_parsing: bool,
    clear_outputs: bool,
    num_columns: usize,
    io: IoPipe,
) -> Result<()> {
    let outline = io.read_outline()?;

    let mut table = Table::new(&outline, num_columns, !no_number_parsing)?;
    table.eval(clear_outputs)?;
    io.write_text(table.to_string())
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
                .filter_map(|((i, c), prev)| {
                    (prev.is_whitespace() && !c.is_whitespace()).then_some(i)
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

    /// Evalaute spreadsheet formulas in all cells and insert their results.
    fn eval(&mut self, clear_outputs: bool) -> Result<()> {
        for row in 0..self.cells.len() {
            for col in 0..self.cells[row].len() {
                if clear_outputs {
                    if self.cells[row][col].is_formula() {
                        self.cells[row][col].assign(NumberValue::empty());
                    }
                } else {
                    self.eval_cell(row, col)?;
                }
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

        // Construct the stack and extract the formula string. The formula
        // marker indicates whether we build a horizontal (from the table row
        // before current cell) or vertical (from the table column above
        // current cell) stack.
        let (mut s, formula) = match c {
            Cell::HorizontalFormula(_, ref f) => {
                (Stack::horizontal(self, row, col), f)
            }
            Cell::VerticalFormula(_, ref f) => {
                (Stack::vertical(self, row, col), f)
            }
            _ => return Ok(()),
        };

        // Must have some stack values to evaluate the formula.
        if s.is_empty() {
            self.cells[row][col].assign(NumberValue::empty());
            return Ok(());
        }

        // Initial stack length, does not change even though formula
        // evaluation may consume and emit stack values.
        let stack_length = s.len();

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
                    let val = s.pop()? + s.pop()?;
                    s.push(val);
                }
                '-' => {
                    // Subtraction
                    let a = s.pop()?;
                    let b = s.pop()?;
                    s.push(b - a);
                }
                '%' | '÷' => {
                    // Division
                    let a = s.pop()?;
                    let b = s.pop()?;
                    s.push(b / a);
                }
                '*' | '×' => {
                    // Multiplication
                    let val = s.pop()? * s.pop()?;
                    s.push(val);
                }
                '·' => {
                    // Drop stack item
                    s.pop()?;
                }
                '⁅' => {
                    // Round to nearest integer.
                    let val = s.pop()?;
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
                    let val = s.pop()?;
                    s.push(val.sqrt());
                }
                '~' => {
                    // Swap top elements
                    let a = s.pop()?;
                    let b = s.pop()?;
                    s.push(a);
                    s.push(b);
                }
                '.' => {
                    // Duplicate top element
                    let a = s.pop()?;
                    s.push(a);
                    s.push(a);
                }
                '²' => {
                    // Square top element
                    let a = s.pop()?;
                    s.push(a * a);
                }
                // If we had reduce (/), sum and product would be shorthand
                // for /+ and /*
                'Σ' => {
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

                c => {
                    bail!("tf: Unsupported formula character '{c}' at ({row}, {col})")
                }
            }
        }
        if let Some(result) = s.top() {
            self.cells[row][col].assign(result);
        } else {
            bail!("tf: Stack underflow at ({row}, {col})")
        }
        Ok(())
    }

    fn column(&self, col: usize) -> impl Iterator<Item = &Cell> + '_ {
        self.cells.iter().filter_map(move |row| row.get(col))
    }
}

impl Display for Table {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Each column's maximum left extension value.
        let left_extents = (0..self.columns)
            .map(|i| {
                self.column(i)
                    .map(|c| c.left_extension())
                    .max()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();

        // Total width for each column
        let column_widths = (0..self.columns)
            .map(|i| {
                self.column(i)
                    .map(|c| c.column_indent(left_extents[i]) + c.len())
                    .max()
                    .unwrap_or(0)
                    + 2 // The 2-space gap between columns
            })
            .collect::<Vec<_>>();

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

                let indent = c.column_indent(left_extents[i]);

                // Pad to meet left pos. To stay IDM-compatible, the leftmost
                // column needs to be padded with NBSPs (\u{00A0}) that don't read as
                // whitespace to IDM.
                if indent > 0 {
                    if i == 0 {
                        write!(f, "{:\u{00A0}^indent$}", "",)?;
                    } else {
                        // Otherwise use spaces.
                        write!(f, "{: ^indent$}", "",)?;
                    }
                }

                write!(f, "{c}")?;

                let right_pad = column_widths[i] - indent - c.len();

                // Right-padding and space between columns.
                if i < row.len() - 1 {
                    write!(f, "{: <right_pad$}", "",)?;
                }
            }

            writeln!(f)?;
        }

        Ok(())
    }
}

struct Stack {
    stack: Vec<f64>,
    is_scientific: bool,
}

impl Stack {
    fn horizontal(table: &Table, row: usize, col: usize) -> Self {
        let mut is_scientific = false;
        let mut stack = Vec::new();

        if let Some(row) = table.cells.get(row) {
            for c in row.iter().take(col) {
                let Some(val) = c.value() else {
                    continue;
                };

                if val.is_scientific() {
                    is_scientific = true;
                }
                stack.push(val.val());
            }
        }

        Stack {
            stack,
            is_scientific,
        }
    }

    fn vertical(table: &Table, row: usize, col: usize) -> Self {
        let mut is_scientific = false;
        let mut stack = Vec::new();

        for r in 0..row {
            if let Some(c) = table.cells.get(r).and_then(|row| row.get(col)) {
                let Some(val) = c.value() else {
                    continue;
                };

                if val.is_scientific() {
                    is_scientific = true;
                }
                stack.push(val.val());
            }
        }

        Stack {
            stack,
            is_scientific,
        }
    }

    fn pop(&mut self) -> Result<f64> {
        self.stack
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Stack underflow"))
    }

    fn top(&self) -> Option<NumberValue> {
        if let Some(&val) = self.stack.last() {
            if self.is_scientific {
                Some(NumberValue::scientific(val))
            } else {
                Some(NumberValue::new(val))
            }
        } else {
            None
        }
    }
}

impl Deref for Stack {
    type Target = Vec<f64>;

    fn deref(&self) -> &Self::Target {
        &self.stack
    }
}

impl DerefMut for Stack {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stack
    }
}

#[derive(Clone, Debug)]
enum Cell {
    Text(String),
    Num(NumberValue),
    VerticalFormula(NumberValue, String),
    HorizontalFormula(NumberValue, String),
}
use Cell::*;

impl FromStr for Cell {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        if let Some((val, form)) = s.split_once(',') {
            if let Ok(val) = val.parse::<NumberValue>() {
                return Ok(HorizontalFormula(val, form.to_string()));
            } else if val.is_empty() {
                return Ok(HorizontalFormula(
                    NumberValue::empty(),
                    form.to_string(),
                ));
            }
        } else if let Some((val, form)) = s.split_once('^') {
            if let Ok(val) = val.parse::<NumberValue>() {
                return Ok(VerticalFormula(val, form.to_string()));
            } else if val.is_empty() {
                return Ok(VerticalFormula(
                    NumberValue::empty(),
                    form.to_string(),
                ));
            }
        } else if let Ok(val) = s.parse::<NumberValue>() {
            return Ok(Num(val));
        }
        Ok(Text(s.to_string()))
    }
}

impl Cell {
    /// Force a text cell, even if the string looks like a number or formula.
    fn text(s: impl Into<String>) -> Self {
        Cell::Text(s.into())
    }

    fn len(&self) -> usize {
        match self {
            Text(s) => s.len(),
            Num(n) => n.as_str().len(),
            VerticalFormula(n, form) => n.as_str().len() + 1 + form.len(),
            HorizontalFormula(n, form) => n.as_str().len() + 1 + form.len(),
        }
    }

    /// How much should this cell be indented when printed to a column wtih
    /// the given maximum left extent.
    fn column_indent(&self, max_left_extent: usize) -> usize {
        if !self.is_numeric() {
            return 0;
        }
        let left_extent = self.left_extension();
        assert!(left_extent <= max_left_extent);
        max_left_extent - left_extent
    }

    fn is_numeric(&self) -> bool {
        !matches!(self, Cell::Text(_))
    }

    fn value(&self) -> Option<&NumberValue> {
        match self {
            Text(_) => None,
            Num(ref n)
            | VerticalFormula(ref n, _)
            | HorizontalFormula(ref n, _) => Some(n),
        }
    }

    fn is_formula(&self) -> bool {
        matches!(self, VerticalFormula(_, _) | HorizontalFormula(_, _))
    }

    fn assign(&mut self, val: NumberValue) {
        match self {
            VerticalFormula(n, _) | HorizontalFormula(n, _) => {
                *n = val;
            }
            cell => {
                *cell = Cell::Num(val);
            }
        }
    }

    /// Find how much the cell must be shifted to the left so it'll align with
    /// other numbers at the exponent marker or the decimal point. Returns 0
    /// for text cells.
    fn left_extension(&self) -> usize {
        let num_part = match self {
            Text(_) => return 0,
            Num(n) | VerticalFormula(n, _) | HorizontalFormula(n, _) => {
                n.as_str()
            }
        };

        if let Some(pos) = num_part.find('e') {
            pos // Try to align by exponent marker first,
        } else if let Some(pos) = num_part.find('.') {
            pos // then by the decimal point,
        } else {
            // Otherwise use the whole number string length.
            num_part.len()
        }
    }
}

impl Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Text(s) => write!(f, "{s}"),
            Num(n) => write!(f, "{n}"),
            VerticalFormula(n, form) => write!(f, "{n}^{form}"),
            HorizontalFormula(n, form) => write!(f, "{n},{form}"),
        }
    }
}

/// Value for numbers that stores the original string representation.
#[derive(Clone, Debug)]
struct NumberValue(f64, String);

impl FromStr for NumberValue {
    type Err = std::num::ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = s.parse::<f64>()?;
        Ok(NumberValue(val, s.to_string()))
    }
}

impl NumberValue {
    /// Construct a new NumberValue with scientific notation in
    /// representation.
    pub fn scientific(val: f64) -> Self {
        let s = format!("{val:.3e}");
        let (n, e) = decompose_float(&s);
        NumberValue(val, format!("{n}{e}"))
    }

    /// Construct a new NumberValue with pretty-printed string representation.
    pub fn new(val: f64) -> Self {
        let s = if 1e-2 > val.abs() && val.abs() > 1e-14 {
            // Always format small nonzero numbers in sci notation.
            format!("{val:.3e}")
        } else {
            // Otherwise do normal number, but only have max three decimal
            // precision, YAGNI more.
            format!("{val:.3}")
        };
        let (n, e) = decompose_float(&s);
        NumberValue(val, format!("{n}{e}"))
    }

    pub fn is_scientific(&self) -> bool {
        self.1.contains('e')
    }

    /// Construct a special NumberValue that evaluates to 0.0 and prints an
    /// empty string.
    pub fn empty() -> Self {
        // This is formally the default value for the type, but it's not
        // declared as Default implementation because printing an empty string
        // is something that should be explicit.
        NumberValue(0.0, String::new())
    }

    pub fn val(&self) -> f64 {
        self.0
    }

    pub fn as_str(&self) -> &str {
        &self.1
    }
}

impl Display for NumberValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.1)
    }
}

/// Split float into truncated decimal part and exponent part.
///
/// "1.234e5" => "1.234", "e5"
/// "1.200e4" => "1.2", "e4" (strip trailing zeroes from float part)
/// "1.000" => "1", "" (strip trailing dot if all decimals are gone)
fn decompose_float(repr: &str) -> (&str, &str) {
    if let Some(pos) = repr.find('e') {
        let (float_part, exp_part) = repr.split_at(pos);
        let float_part = float_part.trim_end_matches('0').trim_end_matches('.');
        (float_part, exp_part)
    } else {
        let float_part = repr.trim_end_matches('0').trim_end_matches('.');
        (float_part, "")
    }
}

// Unit tests

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn tf(input: &str, expected: &str) {
        let outline = idm::from_str(input).unwrap();
        let mut table = Table::new(&outline, 0, true).unwrap();
        table.eval(false).unwrap();
        let output = table.to_string();
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn test_number_value() {
        let n = NumberValue::new(123.456789);
        assert_eq!(n.val(), 123.456789);
        assert_eq!(n.as_str(), "123.457");

        let n2 = NumberValue::new(1.0);
        assert_eq!(n2.val(), 1.0);
        assert_eq!(n2.as_str(), "1");

        let n2 = NumberValue::new(6.674e-11 * 5.972e24 / 6.371e6 / 6.371e6);
        assert_eq!(n2.as_str(), "9.82");

        let n3 = NumberValue::new(0.000123456);
        assert_eq!(n3.val(), 0.000123456);
        assert_eq!(n3.as_str(), "1.235e-4");

        let n3 = NumberValue::new(0.999999);
        assert_eq!(n3.val(), 0.999999);
        assert_eq!(n3.as_str(), "1");
    }

    #[test]
    fn basic_tables() {
        tf(
            "\
1 2 3
4 5 6",
            "\
1  2  3
4  5  6",
        );

        tf(
            "\
a b c d
- 123 - -
e f g h",
            "\
a  b    c  d
-  123  -  -
e  f    g  h",
        );

        // Scientific contagion
        tf("1000000 2000000 ,*", "1000000  2000000  2000000000000,*");

        tf("1e10 2e10 ,*", "1e10  2e10  2e20,*");
    }
}
