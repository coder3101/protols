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
