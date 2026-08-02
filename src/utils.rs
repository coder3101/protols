use async_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Point};

/// Converts a Tree-sitter [`Point`] into an LSP [`Position`].
///
/// This helper maps the row and column coordinates from the syntax tree to the
/// line-and-character coordinate system expected by LSP clients.
///
/// # Saturation Behavior
///
/// Since Tree-sitter defines coordinates using `usize` and the LSP protocol
/// expects `u32`, the values are safely converted using saturating logic. If
/// a coordinate exceeds [`u32::MAX`], it will gracefully saturate to [`u32::MAX`]
/// instead of causing a runtime panic or silent truncation.
#[inline]
pub fn to_lsp_position(Point { row, column }: Point) -> Position {
    Position {
        line: u32::try_from(row).unwrap_or(u32::MAX),
        character: u32::try_from(column).unwrap_or(u32::MAX),
    }
}

/// Converts a Tree-sitter [`Node`] boundary into an LSP [`Range`].
///
/// This helper extracts the line-and-column boundaries from the syntax tree
/// and maps them directly to the coordinate system expected by LSP clients.
#[inline]
pub fn to_lsp_range(node: Node) -> Range {
    let tree_sitter::Range {
        start_point,
        end_point,
        ..
    } = node.range();

    Range {
        start: to_lsp_position(start_point),
        end: to_lsp_position(end_point),
    }
}

#[inline]
pub const fn to_ts_point(Position { line, character }: Position) -> Point {
    Point {
        row: line as usize,
        column: character as usize,
    }
}

/// Evaluates whether a given LSP [`Position`] falls inclusively within the
/// boundaries of an LSP [`Range`].
///
/// This is a bidirectional geometric check ensuring that the cursor or position
/// point is situated both after (or at) the start boundary and before (or at)
/// the end boundary of the range.
#[inline]
pub fn is_position_inside_range(position: Position, range: Range) -> bool {
    position >= range.start && position <= range.end
}

fn is_title_case(s: &str) -> bool {
    s.chars().next().is_some_and(char::is_uppercase)
}

fn is_first_lower_case(s: &&str) -> bool {
    s.chars().next().is_some_and(char::is_lowercase)
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
    let s = s.trim_start_matches('.');
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

/// Strips syntax markers and normalizes whitespace from raw protobuf comment tokens.
///
/// This utility processes both multi-line block comments (`/* ... */`) and single-line
/// trailing comments (`// ...`), returning the clean inner text suitable for markdown rendering
/// in LSP hover cards and documentation tooltips.
///
/// # Examples
///
/// * `"//  My comment"` becomes `" My comment"`
/// * `"// My comment"` becomes `"My comment"`
/// * `"/* Block comment */"` becomes `" Block comment "`
/// * `"Plain text"` remains `"Plain text"`
#[inline]
pub fn clean_proto_comment(raw_text: &str) -> String {
    if let Some(inner) = raw_text.strip_prefix("/*")
        && let Some(uncommented) = inner.strip_suffix("*/")
    {
        return uncommented.to_string();
    }

    if let Some(uncommented) = raw_text.strip_prefix("//") {
        return uncommented
            .strip_prefix(' ')
            .unwrap_or(uncommented)
            .to_string();
    }

    raw_text.to_string()
}

#[cfg(test)]
pub fn compile_test_query() -> tree_sitter::Query {
    let language: tree_sitter::Language = tree_sitter_proto::LANGUAGE.into();

    tree_sitter::Query::new(&language, &crate::model::generate_metamodel_query()).unwrap()
}

#[cfg(test)]
mod test {
    use crate::utils::{
        clean_proto_comment, is_inner_identifier, split_identifier_package, to_lsp_position,
        to_ts_point, trailing_segment,
    };
    use async_lsp::lsp_types::Position;
    use tree_sitter::Point;

    #[test]
    fn test_ts_to_lsp_position() {
        let p = Point { row: 5, column: 10 };
        let pos = to_lsp_position(p);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.character, 10);
    }

    #[test]
    fn test_to_ts_point() {
        let pos = Position {
            line: 3,
            character: 7,
        };
        let p = to_ts_point(pos);
        assert_eq!(p.row, 3);
        assert_eq!(p.column, 7);
    }

    #[test]
    fn test_position_roundtrip() {
        let original = Point {
            row: 42,
            column: 15,
        };
        let pos = to_lsp_position(original);
        let back = to_ts_point(pos);
        assert_eq!(original, back);
    }

    #[test]
    fn test_position_zero() {
        let p = Point { row: 0, column: 0 };
        let pos = to_lsp_position(p);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);
        let back = to_ts_point(pos);
        assert_eq!(p, back);
    }

    #[test]
    fn test_position_large_values() {
        let p = Point {
            row: 999_999,
            column: 999_999,
        };
        let pos = to_lsp_position(p);
        assert_eq!(pos.line, 999_999);
        assert_eq!(pos.character, 999_999);
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

    #[test]
    fn test_clean_proto_comment() {
        assert_eq!(clean_proto_comment("// My comment"), "My comment");
        assert_eq!(clean_proto_comment("//  My comment"), " My comment");
        assert_eq!(clean_proto_comment("//"), "");

        assert_eq!(clean_proto_comment("// My comment\r"), "My comment\r");

        assert_eq!(
            clean_proto_comment("/* Block comment */"),
            " Block comment "
        );
        assert_eq!(
            clean_proto_comment("/*\n * Multi-line\n */"),
            "\n * Multi-line\n "
        );
        assert_eq!(
            clean_proto_comment("/* Block comment\r */"),
            " Block comment\r "
        );

        assert_eq!(
            clean_proto_comment("Plain text documentation"),
            "Plain text documentation"
        );
        assert_eq!(clean_proto_comment(""), "");
    }
}
