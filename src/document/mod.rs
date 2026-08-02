mod parser;

mod diagnostics;
mod docsymbol;
mod hover;
mod rename;
mod syntax;

pub use parser::{ProtoDocument, ProtoParser};
