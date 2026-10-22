use std::path::PathBuf;

use async_lsp::lsp_types::{Position, Range};
use futures::FutureExt;
use tree_sitter::{Node, Point};

/// Converts a Tree-sitter [`Point`] into an LSP [`Position`].
///
/// This helper maps the row and column coordinates from the syntax document to the
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
/// This helper extracts the line-and-column boundaries from the syntax document
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

pub(crate) async fn wait_for_shutdown_signals() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Ctrl+C received");
    }
    .boxed();

    let os_signal = async {
        #[cfg(unix)]
        {
            if let Ok(mut stream) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                stream.recv().await;
                tracing::info!("SIGTERM received");
            }
        }

        #[cfg(windows)]
        {
            if let Ok(mut stream) = tokio::signal::windows::ctrl_close() {
                stream.recv().await;
                tracing::info!("Windows console close event received");
            }
        }

        #[cfg(not(any(unix, windows)))]
        futures::future::pending::<()>().await;
    }
    .boxed();

    let _ = futures::future::select(ctrl_c, os_signal).await;
}

#[derive(Debug, Clone, Default)]
pub struct OutermostPaths(Vec<PathBuf>);

impl OutermostPaths {
    pub fn from_iter(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut raw_paths: Vec<_> = paths.into_iter().collect();

        raw_paths.sort_unstable();
        raw_paths.dedup();

        let mut outermost = Vec::with_capacity(raw_paths.len());
        let mut last_added = None;

        for path in &raw_paths {
            if !last_added.is_some_and(|outer| path.starts_with(outer)) {
                outermost.push(path.clone());
                last_added = Some(path);
            }
        }

        Self(outermost)
    }

    pub fn into_inner(self) -> Vec<PathBuf> {
        self.0
    }

    pub fn iter(&self) -> std::slice::Iter<'_, PathBuf> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl IntoIterator for OutermostPaths {
    type Item = PathBuf;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
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
        trailing_segment,
    };
    use tree_sitter::Point;

    #[test]
    fn test_ts_to_lsp_position() {
        let p = Point { row: 5, column: 10 };
        let pos = to_lsp_position(p);
        assert_eq!(pos.line, 5);
        assert_eq!(pos.character, 10);
    }

    #[test]
    fn test_position_zero() {
        let p = Point { row: 0, column: 0 };
        let pos = to_lsp_position(p);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.character, 0);
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
