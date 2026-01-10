//! Screenshot capture and comparison for terminal testing.
//!
//! This module provides functionality to:
//! - Capture terminal screen state as a "screenshot"
//! - Compare screenshots pixel-by-pixel (cell-by-cell for terminals)
//! - Generate diff output showing differences
//! - Support ignore regions for dynamic content (clock, cursor)
//! - Configure comparison thresholds

use crate::screen::{Cell, CellAttrs, Screen};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// A captured screenshot of the terminal state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screenshot {
    /// Number of columns
    pub cols: usize,
    /// Number of rows
    pub rows: usize,
    /// Grid of cells (row-major order)
    pub cells: Vec<Vec<Cell>>,
    /// Cursor position
    pub cursor: (usize, usize),
    /// Timestamp when captured (in nanoseconds since test start)
    pub timestamp_ns: u64,
}

impl Screenshot {
    /// Create a screenshot from a Screen
    pub fn from_screen(screen: &Screen, timestamp_ns: u64) -> Self {
        let (cols, rows) = screen.size();
        let cursor = screen.cursor();

        let mut cells = Vec::with_capacity(rows);

        for row in 0..rows {
            let mut row_cells = Vec::with_capacity(cols);
            for col in 0..cols {
                if let Some(cell) = screen.get_cell(row, col) {
                    row_cells.push(cell.clone());
                } else {
                    row_cells.push(Cell::new());
                }
            }
            cells.push(row_cells);
        }

        Self {
            cols,
            rows,
            cells,
            cursor: (cursor.row, cursor.col),
            timestamp_ns,
        }
    }

    /// Get a cell at the given position
    pub fn get(&self, row: usize, col: usize) -> Option<&Cell> {
        self.cells.get(row).and_then(|r| r.get(col))
    }

    /// Check if this screenshot has the same dimensions as another
    pub fn same_size(&self, other: &Screenshot) -> bool {
        self.cols == other.cols && self.rows == other.rows
    }
}

/// Region to ignore during comparison
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IgnoreRegion {
    /// Top row (inclusive)
    pub top: usize,
    /// Left column (inclusive)
    pub left: usize,
    /// Bottom row (inclusive)
    pub bottom: usize,
    /// Right column (inclusive)
    pub right: usize,
}

impl IgnoreRegion {
    /// Create a new ignore region
    pub fn new(top: usize, left: usize, bottom: usize, right: usize) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    /// Check if a position is within this region
    pub fn contains(&self, row: usize, col: usize) -> bool {
        row >= self.top && row <= self.bottom && col >= self.left && col <= self.right
    }
}

/// Configuration for screenshot comparison
#[derive(Debug, Clone, PartialEq)]
pub struct DiffConfig {
    /// Regions to ignore during comparison
    pub ignore_regions: Vec<IgnoreRegion>,
    /// Maximum number of different cells to allow
    pub max_differences: usize,
    /// Whether to compare colors
    pub compare_colors: bool,
    /// Whether to compare text
    pub compare_text: bool,
    /// Whether to compare cursor position
    pub compare_cursor: bool,
    /// Character to use for showing differences (for text output)
    pub diff_char: char,
}

impl Default for DiffConfig {
    fn default() -> Self {
        Self {
            ignore_regions: Vec::new(),
            max_differences: 0,
            compare_colors: true,
            compare_text: true,
            compare_cursor: true,
            diff_char: '?',
        }
    }
}

/// Result of a screenshot comparison
#[derive(Debug, Clone, PartialEq)]
pub struct DiffResult {
    /// Whether the screenshots match
    pub matches: bool,
    /// Number of different cells
    pub different_cells: usize,
    /// Maximum difference severity
    pub max_severity: DiffSeverity,
    /// List of all differences
    pub differences: Vec<CellDiff>,
    /// Whether dimensions differed
    pub size_mismatch: bool,
    /// Whether cursor positions differed
    pub cursor_mismatch: bool,
    /// Similarity ratio (0.0 to 1.0)
    pub similarity: f64,
}

/// Severity of a cell difference
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiffSeverity {
    /// Cells are identical
    Identical,
    /// Only text differs
    TextOnly,
    /// Only attributes differ
    AttrOnly,
    /// Both text and attributes differ
    Full,
}

/// A single cell difference
#[derive(Debug, Clone, PartialEq)]
pub struct CellDiff {
    /// Row position
    pub row: usize,
    /// Column position
    pub col: usize,
    /// Severity of difference
    pub severity: DiffSeverity,
    /// Expected cell (from baseline)
    pub expected: Cell,
    /// Actual cell (from new screenshot)
    pub actual: Cell,
}

/// Compare two screenshots
pub fn compare_screenshots(
    baseline: &Screenshot,
    actual: &Screenshot,
    config: &DiffConfig,
) -> DiffResult {
    // Check size mismatch first
    let size_mismatch = baseline.cols != actual.cols || baseline.rows != actual.rows;

    if size_mismatch {
        return DiffResult {
            matches: false,
            different_cells: 0,
            max_severity: DiffSeverity::Full,
            differences: Vec::new(),
            size_mismatch: true,
            cursor_mismatch: false,
            similarity: 0.0,
        };
    }

    // Check cursor mismatch
    let cursor_mismatch = if config.compare_cursor {
        baseline.cursor != actual.cursor
    } else {
        false
    };

    // Find all differences
    let mut differences = Vec::new();
    let mut total_cells = baseline.cols * baseline.rows;
    let mut matching_cells = 0;
    let empty_cell = Cell::new();

    for row in 0..baseline.rows {
        for col in 0..baseline.cols {
            // Check if this cell is in an ignore region
            if config.ignore_regions.iter().any(|r| r.contains(row, col)) {
                continue;
            }

            let baseline_cell = baseline.get(row, col).unwrap_or(&empty_cell);
            let actual_cell = actual.get(row, col).unwrap_or(&empty_cell);

            let severity = compute_severity(baseline_cell, actual_cell, config);

            if severity != DiffSeverity::Identical {
                differences.push(CellDiff {
                    row,
                    col,
                    severity,
                    expected: baseline_cell.clone(),
                    actual: actual_cell.clone(),
                });
            } else {
                matching_cells += 1;
            }
        }
    }

    let different_cells = differences.len();
    let max_severity = differences
        .iter()
        .map(|d| d.severity)
        .max()
        .unwrap_or(DiffSeverity::Identical);

    let similarity = if total_cells > 0 {
        matching_cells as f64 / total_cells as f64
    } else {
        1.0
    };

    let matches = different_cells <= config.max_differences && !cursor_mismatch;

    DiffResult {
        matches,
        different_cells,
        max_severity,
        differences,
        size_mismatch: false,
        cursor_mismatch,
        similarity,
    }
}

/// Compute the severity of difference between two cells
fn compute_severity(baseline: &Cell, actual: &Cell, config: &DiffConfig) -> DiffSeverity {
    let text_diff = config.compare_text && baseline.ch != actual.ch;
    let attr_diff = config.compare_colors && baseline.attrs != actual.attrs;

    match (text_diff, attr_diff) {
        (false, false) => DiffSeverity::Identical,
        (true, false) => DiffSeverity::TextOnly,
        (false, true) => DiffSeverity::AttrOnly,
        (true, true) => DiffSeverity::Full,
    }
}

/// Generate a visual diff output showing differences
pub fn generate_diff_output(baseline: &Screenshot, actual: &Screenshot) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!(
        "Screenshot Diff ({}x{})\n",
        baseline.cols, baseline.rows
    ));
    output.push_str(&format!("Expected cursor: {:?}\n", baseline.cursor));
    output.push_str(&format!("Actual cursor: {:?}\n\n", actual.cursor));

    // Size mismatch
    if baseline.cols != actual.cols || baseline.rows != actual.rows {
        output.push_str(&format!(
            "SIZE MISMATCH: baseline={}x{}, actual={}x{}\n",
            baseline.cols, baseline.rows, actual.cols, actual.rows
        ));
        return output;
    }

    // Show differences
    output.push_str("Diff (baseline vs actual):\n");
    output.push_str(&"=".repeat(baseline.cols.min(80)));
    output.push('\n');

    let empty_cell = Cell::new();
    for row in 0..baseline.rows {
        for col in 0..baseline.cols {
            let baseline_cell = baseline.get(row, col).unwrap_or(&empty_cell);
            let actual_cell = actual.get(row, col).unwrap_or(&empty_cell);

            let ch = if baseline_cell == actual_cell {
                baseline_cell.ch
            } else {
                '?'
            };
            output.push(ch);
        }
        output.push('\n');
    }

    output.push_str(&"=".repeat(baseline.cols.min(80)));
    output.push('\n');

    // Legend
    output.push_str("\nLegend:\n");
    output.push_str("  ? = different cell\n");
    output.push_str("  [other char] = same as baseline\n");

    // Summary
    let mut diff_count = 0;
    let empty_cell = Cell::new();
    for row in 0..baseline.rows {
        for col in 0..baseline.cols {
            let baseline_cell = baseline.get(row, col).unwrap_or(&empty_cell);
            let actual_cell = actual.get(row, col).unwrap_or(&empty_cell);
            if baseline_cell != actual_cell {
                diff_count += 1;
            }
        }
    }

    output.push_str(&format!("\nTotal different cells: {}\n", diff_count));

    output
}

/// Calculate structural similarity between two screenshots
pub fn structural_similarity(baseline: &Screenshot, actual: &Screenshot) -> f64 {
    if baseline.cols != actual.cols || baseline.rows != actual.rows {
        return 0.0;
    }

    let total_cells = baseline.cols * baseline.rows;
    let mut identical_cells = 0.0;
    let empty_cell = Cell::new();

    for row in 0..baseline.rows {
        for col in 0..baseline.cols {
            let baseline_cell = baseline.get(row, col).unwrap_or(&empty_cell);
            let actual_cell = actual.get(row, col).unwrap_or(&empty_cell);

            // Check text similarity
            let text_match = baseline_cell.ch == actual_cell.ch;

            // Check attribute similarity
            let attr_match = baseline_cell.attrs == actual_cell.attrs;

            if text_match && attr_match {
                identical_cells += 1.0;
            } else if text_match {
                // Partial match - text same but attributes different
                identical_cells += 0.5;
            }
        }
    }

    identical_cells / total_cells as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cell(ch: char, fg: i16, bg: i16) -> Cell {
        Cell {
            ch,
            attrs: CellAttrs {
                fg,
                bg,
                flags: Default::default(),
            },
        }
    }

    fn make_screenshot(cells: Vec<Vec<Cell>>, cursor: (usize, usize)) -> Screenshot {
        let rows = cells.len();
        let cols = if rows > 0 { cells[0].len() } else { 0 };
        Screenshot {
            cols,
            rows,
            cells,
            cursor,
            timestamp_ns: 0,
        }
    }

    #[test]
    fn test_identical_screenshots_match() {
        let cell = make_cell('x', 1, 2);
        let screenshot = make_screenshot(vec![vec![cell.clone()]], (0, 0));

        let result = compare_screenshots(&screenshot, &screenshot, &DiffConfig::default());

        assert!(result.matches);
        assert_eq!(result.different_cells, 0);
        assert_eq!(result.similarity, 1.0);
    }

    #[test]
    fn test_different_text_detected() {
        let baseline = make_screenshot(vec![vec![make_cell('a', 1, 2)]], (0, 0));
        let actual = make_screenshot(vec![vec![make_cell('b', 1, 2)]], (0, 0));

        let result = compare_screenshots(&baseline, &actual, &DiffConfig::default());

        assert!(!result.matches);
        assert_eq!(result.different_cells, 1);
        assert_eq!(result.max_severity, DiffSeverity::TextOnly);
    }

    #[test]
    fn test_different_colors_detected() {
        let baseline = make_screenshot(vec![vec![make_cell('a', 1, 2)]], (0, 0));
        let actual = make_screenshot(vec![vec![make_cell('a', 3, 2)]], (0, 0));

        let result = compare_screenshots(&baseline, &actual, &DiffConfig::default());

        assert!(!result.matches);
        assert_eq!(result.different_cells, 1);
        assert_eq!(result.max_severity, DiffSeverity::AttrOnly);
    }

    #[test]
    fn test_ignore_region() {
        let baseline = make_screenshot(
            vec![
                vec![make_cell('a', 1, 2), make_cell('b', 1, 2)],
                vec![make_cell('c', 1, 2), make_cell('d', 1, 2)],
            ],
            (0, 0),
        );
        let actual = make_screenshot(
            vec![
                vec![make_cell('x', 3, 4), make_cell('b', 1, 2)],
                vec![make_cell('c', 1, 2), make_cell('y', 5, 6)],
            ],
            (0, 0),
        );

        let mut config = DiffConfig::default();
        config.ignore_regions.push(IgnoreRegion::new(0, 0, 0, 0)); // Ignore top-left cell

        let result = compare_screenshots(&baseline, &actual, &config);

        // Only the bottom-right cell should be different (top-left is ignored)
        assert!(!result.matches);
        assert_eq!(result.different_cells, 1);
    }

    #[test]
    fn test_max_differences_threshold() {
        let baseline = make_screenshot(
            vec![vec![make_cell('a', 1, 2), make_cell('b', 1, 2)]],
            (0, 0),
        );
        let actual = make_screenshot(
            vec![vec![make_cell('x', 3, 4), make_cell('y', 5, 6)]],
            (0, 0),
        );

        let mut config = DiffConfig::default();
        config.max_differences = 1;

        let result = compare_screenshots(&baseline, &actual, &config);

        // 2 differences, but max is 1
        assert!(!result.matches);
        assert_eq!(result.different_cells, 2);
    }

    #[test]
    fn test_cursor_mismatch() {
        let baseline = make_screenshot(vec![vec![make_cell('a', 1, 2)]], (0, 0));
        let actual = make_screenshot(vec![vec![make_cell('a', 1, 2)]], (0, 1));

        let result = compare_screenshots(&baseline, &actual, &DiffConfig::default());

        assert!(!result.matches);
        assert!(result.cursor_mismatch);
    }

    #[test]
    fn test_size_mismatch() {
        let baseline = make_screenshot(vec![vec![make_cell('a', 1, 2)]], (0, 0));
        let actual = make_screenshot(
            vec![vec![make_cell('a', 1, 2)], vec![make_cell('b', 1, 2)]],
            (0, 0),
        );

        let result = compare_screenshots(&baseline, &actual, &DiffConfig::default());

        assert!(!result.matches);
        assert!(result.size_mismatch);
        assert_eq!(result.similarity, 0.0);
    }

    #[test]
    fn test_structural_similarity() {
        let baseline = make_screenshot(
            vec![
                vec![make_cell('a', 1, 2), make_cell('b', 1, 2)],
                vec![make_cell('c', 1, 2), make_cell('d', 1, 2)],
            ],
            (0, 0),
        );
        let actual = make_screenshot(
            vec![
                vec![make_cell('a', 1, 2), make_cell('b', 3, 4)],
                vec![make_cell('c', 1, 2), make_cell('d', 1, 2)],
            ],
            (0, 0),
        );

        let sim = structural_similarity(&baseline, &actual);

        // 3 cells identical (a,c,d), 1 cell text match attrs different (b->b with diff attrs)
        // = (3 + 0.5) / 4 = 0.875
        assert!((sim - 0.875).abs() < 0.001);
    }

    #[test]
    fn test_generate_diff_output() {
        let baseline = make_screenshot(vec![vec![make_cell('a', 1, 2)]], (0, 0));
        let actual = make_screenshot(vec![vec![make_cell('b', 3, 4)]], (0, 0));

        let output = generate_diff_output(&baseline, &actual);

        assert!(output.contains("1x1"));
        assert!(output.contains("Expected cursor"));
        assert!(output.contains("Actual cursor"));
        assert!(output.contains("?"));
    }
}
