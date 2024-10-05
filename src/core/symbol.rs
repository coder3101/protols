use async_lsp::lsp_types::Position;
use serde::Serialize;
use tree_sitter::{Point, Range};

#[derive(Debug, Serialize)]
pub enum ProtoSymbolKind {
    Message,
    Enum,
}

#[derive(Debug, Serialize)]
pub struct ProtoSymbolPoint {
    row: usize,
    col: usize,
}

#[derive(Debug, Serialize)]
pub struct ProtoSymbolRange {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: ProtoSymbolPoint,
    pub end_point: ProtoSymbolPoint,
}

#[derive(Debug, Serialize)]
pub struct ProtoSymbol {
    pub kind: ProtoSymbolKind,
    pub text: String,
    pub range: ProtoSymbolRange,
}

impl From<Point> for ProtoSymbolPoint {
    fn from(value: Point) -> Self {
        Self {
            row: value.row,
            col: value.column,
        }
    }
}

impl From<ProtoSymbolPoint> for Position {
    fn from(value: ProtoSymbolPoint) -> Self {
        Self {
            line: value.row as u32,
            character: value.col as u32,
        }
    }
}

impl From<ProtoSymbolPoint> for Point {
    fn from(value: ProtoSymbolPoint) -> Self {
        Self {
            row: value.row,
            column: value.col,
        }
    }
}

impl From<Range> for ProtoSymbolRange {
    fn from(value: Range) -> Self {
        Self {
            start_byte: value.start_byte,
            end_byte: value.end_byte,
            start_point: value.start_point.into(),
            end_point: value.end_point.into(),
        }
    }
}
