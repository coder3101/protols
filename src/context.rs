#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Jumpable {
    Import(String),
    Identifier(String),
}

