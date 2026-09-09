//! Source-context diagnostics: render an error with the offending source
//! line and a caret pointer, using the lexer/parser line/column spans.

/// Render a diagnostic for `line:col` (1-based) in `source`.
///
/// ```text
/// examples/foo.sbx:3:14: Unexpected token '}' in expression
///     let x = 1 + }
///              ^
/// ```
///
/// Falls back to a bare `file: message` when the position is unknown
/// (line 0) or the line is past EOF.
pub fn render(source: &str, filename: &str, line: usize, col: usize, message: &str) -> String {
    if line == 0 {
        return format!("{filename}: {message}");
    }
    let Some(line_text) = source.lines().nth(line - 1) else {
        return format!("{filename}:{line}:{col}: {message}");
    };
    // Caret offset in display columns: the offending token starts at `col`,
    // but preceding tabs widen the visible line.
    let col_clamped = col.clamp(1, line_text.chars().count() + 1);
    let caret_col: usize = line_text
        .chars()
        .take(col_clamped - 1)
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum();
    format!(
        "{filename}:{line}:{col}: {message}\n    {line_text}\n    {caret}^",
        caret = " ".repeat(caret_col),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_under_token() {
        let src = "fn main() {\n    let x = 1 +\n}\n";
        let out = render(src, "t.sbx", 2, 15, "Unexpected token");
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("t.sbx:2:15:"), "{}", lines[0]);
        assert_eq!(lines[1], "        let x = 1 +"); // 4-space prefix + source line
                                                     // col 15 = the '+'; caret index = 4 (prefix) + 14 (col-1)
        assert_eq!(lines[2].find('^'), Some(4 + 14));
    }

    #[test]
    fn tab_expansion() {
        let src = "\tif x {\n}\n";
        let out = render(src, "t.sbx", 1, 7, "Unexpected token");
        let lines: Vec<&str> = out.lines().collect();
        // tab widens to 4 columns: caret index = 4 (prefix) + 4 (tab) + 1 ('{' is col 6 -> col-1=5... )
        // col 7 = '{'; preceding chars: tab(4) + "if x " (5) = 9; caret index = 4 + 9
        assert_eq!(lines[2].find('^'), Some(4 + 9));
    }

    #[test]
    fn line_past_eof_degrades() {
        let out = render("fn main() {}", "t.sbx", 9, 1, "boom");
        assert_eq!(out, "t.sbx:9:1: boom");
    }

    #[test]
    fn unknown_position_degrades() {
        let out = render("fn main() {}", "t.sbx", 0, 0, "boom");
        assert_eq!(out, "t.sbx: boom");
    }
}
