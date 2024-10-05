use std::sync::{Arc, LazyLock, Mutex};

use anyhow::{anyhow, Result};
use tree_sitter::{Node, Parser, Query, QueryCursor, Tree};

use crate::{core::query::QUERY_SYMBOLS, nodekind::NodeKind};

use super::symbol::{ProtoSymbol, ProtoSymbolKind};

static PARSER: LazyLock<Mutex<Parser>> = LazyLock::new(|| {
    let mut parser = Parser::new();
    parser
        .set_language(&protols_tree_sitter_proto::language())
        .expect("failed to initialise parser");
    Mutex::new(parser)
});

#[derive(Clone)]
pub(super) struct ProtoAST {
    inner: Arc<Tree>,
}

impl ProtoAST {
    fn matches_s_expr(n: Node<'_>, expr: &[&str]) -> bool {
        let Some((kind, rest)) = expr.split_last() else {
            return true;
        };

        if n.kind() != *kind {
            return false;
        }

        if let Some(p) = n.parent() {
            Self::matches_s_expr(p, rest)
        } else {
            return false;
        }
    }
    
    fn symbol_name<'b>(&self, n: Node<'_>, content: &'b str) -> Option<&'b str> {
        let mut cursor = n.walk();
        let child = n.named_children(&mut cursor).find(NodeKind::is_userdefined);
        child.and_then(|c| c.utf8_text(content.as_bytes()).ok())
    }

    fn parent_name(&self, n: Node<'_>, content: &str) -> Option<String> {
        let mut i = n;
        let mut r: Vec<&str> = vec![];

        while let Some(pp) = i.parent() {
            if NodeKind::is_message(&pp) {
                if let Some(name) = self.symbol_name(pp, content) {
                    r.push(name);
                }
            }
            i = pp;
        }

        if !r.is_empty() {
            r.reverse();
            Some(r.join("."))
        } else {
            None
        }
    }
}

impl ProtoAST {
    pub(super) fn new(s: &str) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(
                PARSER
                    .lock()
                    .expect("parser poisoned")
                    .parse(s, None)
                    .ok_or_else(|| anyhow!("failed to parse the file"))?,
            ),
        })
    }

    pub(super) fn update(&mut self, s: &str) -> Result<()> {
        let new_ast = PARSER
            .lock()
            .expect("parser poisoned")
            .parse(s, Some(self.inner.as_ref()))
            .ok_or_else(|| anyhow!("failed to parse the file"))?;

        self.inner = Arc::new(new_ast);
        Ok(())
    }

    pub(super) fn query<'a>(&'a self, q: &Query, content: &str) -> Vec<Node<'a>> {
        let mut qc = QueryCursor::new();
        qc.matches(q, self.inner.root_node(), content.as_bytes())
            .map(|qm| qm.captures)
            .flatten()
            .map(|c| c.node)
            .collect()
    }

    pub(super) fn symbols(&self, content: &str) -> Vec<ProtoSymbol> {
        let mut qc = QueryCursor::new();
        qc.matches(&QUERY_SYMBOLS, self.inner.root_node(), content.as_bytes())
            .map(|qm| (qm.captures[0].node, qm.captures[1].node))
            .map(|(s, id)| {
                let identifier_name = id
                    .utf8_text(content.as_bytes())
                    .expect("utf8 parser error");

                let full_name = if let Some(parent_name) = self.parent_name(s, content) {
                    parent_name + "." + identifier_name
                } else {
                    identifier_name.to_string()
                };

                ProtoSymbol {
                    kind: if NodeKind::is_message(&s) {
                        ProtoSymbolKind::Message
                    } else {
                        ProtoSymbolKind::Enum
                    },
                    text: full_name,
                    range: s.range().into(),
                }
            })
            .collect()
    }
}
