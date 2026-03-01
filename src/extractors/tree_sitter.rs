// Author: kelexine (https://github.com/kelexine)
// extractors/tree_sitter.rs — Generic tree-sitter extraction logic

use super::{Extractor, estimate_complexity};
use crate::models::FunctionInfo;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};
use std::collections::HashMap;

/// A generic extractor that uses tree-sitter queries to identify functions and classes.
// TODO: Flesh out and complete implementation
#[allow(dead_code)]
pub struct TreeSitterExtractor {
    language: Language,
    query: Query,
}

#[allow(dead_code)]
impl TreeSitterExtractor {
    /// Creates a new TreeSitterExtractor for a given language and query string.
    /// The query string should define captures like @function, @name, @class, @method.
    pub fn new(language: Language, query_source: &str) -> Result<Self, tree_sitter::QueryError> {
        let query = Query::new(&language, query_source)?;
        Ok(Self { language, query })
    }
}

impl Extractor for TreeSitterExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        let mut parser = Parser::new();
        if parser.set_language(&self.language).is_err() {
            return vec![];
        }

        let tree = match parser.parse(content, None) {
            Some(tree) => tree,
            None => return vec![],
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&self.query, tree.root_node(), content.as_bytes());

        let lines: Vec<&str> = content.lines().collect();
        let mut functions = HashMap::new();

        let capture_names = self.query.capture_names();
        let mut capture_map = HashMap::new();
        for (i, name) in capture_names.iter().enumerate() {
            capture_map.insert(*name, i as u32);
        }

        while let Some(m) = matches.next() {
            let mut root_node = None;
            let mut name = String::new();
            let mut is_class = false;
            let mut is_method = false;

            for cap in m.captures {
                let capture_name = capture_names[cap.index as usize];
                let text = cap.node.utf8_text(content.as_bytes()).unwrap_or("").to_string();

                match capture_name {
                    "function" | "class" | "method" => {
                        root_node = Some(cap.node);
                        if capture_name == "class" {
                            is_class = true;
                        }
                        if capture_name == "method" {
                            is_method = true;
                        }
                    }
                    "name" => {
                        name = text;
                    }
                    _ => {}
                }
            }

            if let Some(node) = root_node
                && !name.is_empty()
            {
                let start_point = node.start_position();
                let end_point = node.end_position();
                
                let line_start = start_point.row + 1;
                let line_end = end_point.row + 1;
                
                // Fallback to simple complexity estimation for now
                // TODO: Flesh out and complete implementation
                let block = &lines[line_start.saturating_sub(1)..line_end.min(lines.len())];
                let complexity = if is_class { 1 } else { estimate_complexity(block) };

                let info = FunctionInfo {
                    name: name.clone(),
                    line_start,
                    line_end,
                    parameters: vec![], // TODO: Expand this logic as needed for params
                    is_async: false,    // TODO: Expand this logic for async
                    is_method,
                    is_class,
                    docstring: None,
                    decorators: vec![],
                    complexity,
                };

                // Use node id to avoid duplicates if multiple queries match the same node
                functions.insert(node.id(), info);
            }
        }

        let mut result: Vec<_> = functions.into_values().collect();
        result.sort_by_key(|f| f.line_start);
        result
    }
}
