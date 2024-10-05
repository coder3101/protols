use std::iter::zip;

pub(super) fn char_to_byte(line: &str, char: u32) -> usize {
    line.chars().take(char as usize).map(char::len_utf8).sum()
}

pub(super) fn relativise<'a>(base: &str, full: &'a str) -> &'a str {
    let common_prefix = zip(base.split('.'), full.split('.'))
        .take_while(|(b, f)| b == f)
        .map(|(_, f)| f)
        .collect::<Vec<_>>()
        .join(".");

    let n = common_prefix.len();

    if full.starts_with(&common_prefix) && full.len() > n && n != 0 {
        &full[n + 1..]
    } else if full.len() == n && full.contains('.') {
        full.rsplit_once(".").unwrap().1
    } else {
        full
    }
}

#[cfg(test)]
mod test {
    use crate::core::utils::{char_to_byte, relativise};

    #[test]
    fn test_char_to_bytes() {
        assert_eq!(char_to_byte("abc", 1), 1);
        assert_eq!(char_to_byte("it is alpha, 𝛼bc", 14), 17);
    }

    #[test]
    fn test_relativise() {
        assert_eq!(relativise("Foo", "Foo.Bar"), "Bar");
        assert_eq!(relativise("Fot", "Foo.Bar"), "Foo.Bar");
        assert_eq!(relativise("Foo.Bar", "Foo.Bar"), "Bar");
        assert_eq!(relativise("Foo.Bar.Baz", "Foo.Bar"), "Bar");
        assert_eq!(relativise("Foo.Bar", "Foo.Bar.Baz"), "Baz");
        assert_eq!(relativise("Foo.Bar", "Foo.Bar.Baz.Dit"), "Baz.Dit");
    }
}
