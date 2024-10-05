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

pub fn split_identifier_package(s: &str) -> (&str, &str) {
    // trim beginning dots, some use `.` prefix for fully qualified field names
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
    return (package, identifier.trim_matches('.'));
}

#[cfg(test)]
mod test {
    use crate::utils::{is_inner_identifier, split_identifier_package};

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
}
