use async_lsp::lsp_types::Position;
use tree_sitter::Point;

pub fn ts_to_lsp_position(p: &Point) -> Position {
    Position {
        line: p.row as u32,
        character: p.column as u32,
    }
}

pub fn lsp_to_ts_point(p: &Position) -> Point {
    Point {
        row: p.line as usize,
        column: p.character as usize,
    }
}

fn is_title_case(s: &str) -> bool {
    s.chars()
        .next()
        .map(|x| x.is_uppercase())
        .unwrap_or_default()
}

fn is_first_lower_case(s: &&str) -> bool {
    s.chars()
        .next()
        .map(|x| x.is_lowercase())
        .unwrap_or_default()
}

pub fn is_inner_identifier(s: &str) -> bool {
    if !s.contains('.') {
        return false;
    }
    s.split('.').all(is_title_case)
}

/// Returns the segment after the last `.` in a dotted identifier, or the whole
/// string if it contains no dot.
pub fn trailing_segment(qualified: &str) -> &str {
    qualified.rsplit_once('.').map_or(qualified, |(_, t)| t)
}

pub fn split_identifier_package(s: &str) -> (&str, &str) {
    let s = s.trim_start_matches(".");
    if is_inner_identifier(s) || !s.contains('.') {
        return ("", s);
    }

    let i = s
        .split('.')
        .take_while(is_first_lower_case)
        .fold(0, |mut c, s| {
            if c != 0 {
                c += 1;
            }
            c += s.len();
            c
        });

    let (package, identifier) = s.split_at(i);
    (package, identifier.trim_matches('.'))
}

#[cfg(test)]
mod test {
    use crate::utils::{
        is_inner_identifier, lsp_to_ts_point, split_identifier_package, trailing_segment,
        ts_to_lsp_position,
    };
    use async_lsp::lsp_types::Position;
    use tree_sitter::Point;

    #[test]
    fn test_ts_to_lsp_position() {
        let p = Point { row: 5, column: 10 };
        let pos = ts_to_lsp_position(&p);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.character, 10);
    }

    #[test]
    fn test_lsp_to_ts_point() {
        let pos = Position {
            line: 3,
            character: 7,
        };
        let p = lsp_to_ts_point(&pos);
        assert_eq!(p.row, 3);
        assert_eq!(p.column, 7);
    }

    #[test]
    fn test_position_roundtrip() {
        let original = Point {
            row: 42,
            column: 15,
        };
        let pos = ts_to_lsp_position(&original);
        let back = lsp_to_ts_point(&pos);
        assert_eq!(original, back);
    }

    #[test]
    fn test_position_zero() {
        let p = Point { row: 0, column: 0 };
        let pos = ts_to_lsp_position(&p);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);
        let back = lsp_to_ts_point(&pos);
        assert_eq!(p, back);
    }

    #[test]
    fn test_position_large_values() {
        let p = Point {
            row: 999999,
            column: 999999,
        };
        let pos = ts_to_lsp_position(&p);
        assert_eq!(pos.line, 999999);
        assert_eq!(pos.character, 999999);
    }

    #[test]
    fn test_trailing_segment() {
        assert_eq!(trailing_segment("Foo"), "Foo");
        assert_eq!(trailing_segment("foo.Bar"), "Bar");
        assert_eq!(trailing_segment("foo.bar.Baz"), "Baz");
        assert_eq!(trailing_segment(".foo.Bar"), "Bar");
        assert_eq!(trailing_segment(""), "");
    }

    #[test]
    fn test_is_inner_identifier() {
        assert!(is_inner_identifier("Book.Author"));
        assert!(is_inner_identifier("Book.Author.Address"));

        assert!(!is_inner_identifier("com.book.Foo"));
        assert!(!is_inner_identifier("Book"));
        assert!(!is_inner_identifier("foo.Bar"));
    }

    #[test]
    fn test_split_identifier_package() {
        assert_eq!(
            split_identifier_package("com.book.Book"),
            ("com.book", "Book")
        );
        assert_eq!(
            split_identifier_package(".com.book.Book"),
            ("com.book", "Book")
        );
        assert_eq!(
            split_identifier_package("com.book.Book.Author"),
            ("com.book", "Book.Author")
        );

        assert_eq!(split_identifier_package("com.Book"), ("com", "Book"));
        assert_eq!(split_identifier_package("Book"), ("", "Book"));
        assert_eq!(split_identifier_package("Book.Author"), ("", "Book.Author"));
        assert_eq!(split_identifier_package("com.book"), ("com.book", ""));
    }

    #[test]
    fn test_split_identifier_package_single_segment_package() {
        assert_eq!(split_identifier_package("foo.Bar"), ("foo", "Bar"));
        assert_eq!(split_identifier_package("a.B.C"), ("a", "B.C"));
    }

    #[test]
    fn test_split_identifier_package_leading_dot() {
        assert_eq!(split_identifier_package(".foo.bar.Baz"), ("foo.bar", "Baz"));
        assert_eq!(split_identifier_package(".Bar"), ("", "Bar"));
    }

    #[test]
    fn test_split_identifier_package_all_lowercase() {
        assert_eq!(
            split_identifier_package("com.example.package"),
            ("com.example.package", "")
        );
    }

    #[test]
    fn test_split_identifier_package_all_uppercase() {
        assert_eq!(split_identifier_package("Foo.Bar"), ("", "Foo.Bar"));
    }

    #[test]
    fn test_split_identifier_package_mixed_case_segments() {
        assert_eq!(
            split_identifier_package("my.pkg.MyMessage"),
            ("my.pkg", "MyMessage")
        );
        assert_eq!(
            split_identifier_package("org.example.api.V1.Request"),
            ("org.example.api", "V1.Request")
        );
    }
}
