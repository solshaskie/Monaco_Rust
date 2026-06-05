use serde::{Deserialize, Serialize};

/// An edit operation produced by the diff engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffEdit {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub text: String,
}

/// Compute a simple line-based diff between old_text and new_text.
/// Uses a basic longest-common-subsequence (LCS) approach.
/// Returns a sequence of edits that transform old_text into new_text.
pub fn line_diff(old_text: &str, new_text: &str) -> Vec<DiffEdit> {
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    let lcs = compute_lcs(&old_lines, &new_lines);
    let mut edits = Vec::new();
    let mut old_i = 0usize;
    let mut new_i = 0usize;

    while old_i < old_lines.len() || new_i < new_lines.len() {
        if old_i < old_lines.len() && new_i < new_lines.len() && old_lines[old_i] == new_lines[new_i] {
            old_i += 1;
            new_i += 1;
        } else if old_i < old_lines.len()
            && (new_i >= new_lines.len() || !lcs.contains(&old_lines[old_i]))
        {
            // Line deleted
            let line_num = (old_i + 1) as u32;
            let end_line = if old_i + 1 < old_lines.len() {
                line_num + 1
            } else {
                line_num
            };
            edits.push(DiffEdit {
                start_line: line_num,
                start_column: 1,
                end_line,
                end_column: 1,
                text: String::new(),
            });
            old_i += 1;
        } else {
            // Line inserted
            let line_num = (old_i + 1) as u32;
            let text = format!("{}\n", new_lines[new_i]);
            edits.push(DiffEdit {
                start_line: line_num,
                start_column: 1,
                end_line: line_num,
                end_column: 1,
                text,
            });
            new_i += 1;
        }
    }

    edits
}

fn compute_lcs<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<&'a str> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }

    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];

    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            if a[i] == b[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    let mut result = Vec::new();
    let mut i = 0;
    let mut j = 0;
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            result.push(a[i]);
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_no_change() {
        let edits = line_diff("hello\nworld", "hello\nworld");
        assert!(edits.is_empty());
    }

    #[test]
    fn diff_insert_line() {
        let edits = line_diff("hello", "hello\nworld");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text, "world\n");
    }

    #[test]
    fn diff_delete_line() {
        let edits = line_diff("hello\nworld", "hello");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].text, "");
    }

    #[test]
    fn diff_replace_line() {
        let edits = line_diff("hello\nworld", "hello\nuniverse");
        assert!(!edits.is_empty());
    }
}
