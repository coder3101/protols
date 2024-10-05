#[derive(Debug)]
pub enum CompletionContext<'a> {
    Message(&'a str),
    Enum(&'a str),
    Import,
    Keyword,
    Syntax,
    Option,
}

#[derive(Debug)]
pub struct GotoTypeContext<'a> {
    pub name: &'a str,
    pub parent: Option<String>,
}

#[derive(Debug)]
pub enum GotoContext<'a> {
    Type(GotoTypeContext<'a>),
    Import(&'a str),
}
