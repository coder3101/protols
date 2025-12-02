use std::path::Path;

fn main() {
    if let Some(path) = option_env!("FALLBACK_INCLUDE_PATH") {
        let path = Path::new(path);
        assert!(
            path.is_absolute(),
            "Environment variable FALLBACK_INCLUDE_PATH must be absolute: {path:?}"
        );
    }
}
