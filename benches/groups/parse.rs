// Author: kelexine <https://github.com/kelexine>
// groups/parse.rs — Phase breakdown of function extraction (parse vs. tree walks).
//
// Profiling shows tree-sitter parsing dominates `extractors/extract` (roughly
// 70–90%), with the complexity walk a distant second.  This group isolates those
// phases so a regression in one cannot hide inside the combined extractor number:
//
// - `parse`           — `with_parsed_tree`: thread-local parser reuse + parse + drop
// - `complexity_walk` — one `ast_complexity` pass over the whole tree
// - `walk_floor`      — bare DFS touching only `node.kind()`; the minimum any walk costs
//
// The grammar table below mirrors what `extractors::get_extractor` ships.  In
// particular `.c` files are parsed with the C++ grammar, exactly as the binary does.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use tree_sitter::{Language, Node, Tree};

use crate::extractors::tree_sitter::ast_complexity;
use crate::extractors::with_parsed_tree;
use crate::support::fixtures::{self, Fixture};

const LINES: usize = 10_000;

/// Grammar used by the shipped extractor for each plain-source fixture.
fn grammar(fixture: &Fixture) -> Language {
    match fixture.name {
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        // The binary routes `.c` and `.cpp` through the same C++ grammar.
        "c" | "cpp" => tree_sitter_cpp::LANGUAGE.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "php" => tree_sitter_php::LANGUAGE_PHP.into(),
        "ruby" => tree_sitter_ruby::LANGUAGE.into(),
        "swift" => tree_sitter_swift::LANGUAGE.into(),
        other => panic!("no grammar mapped for fixture `{other}`"),
    }
}

/// One language's prepared input plus its pre-parsed tree.
struct Case {
    name: &'static str,
    language: Language,
    content: String,
    tree: Tree,
}

/// Count every node with an iterative DFS that only reads `kind()`.
fn walk_floor(root: Node<'_>) -> usize {
    let mut count = 0usize;
    let mut cursor = root.walk();
    let mut visited = false;

    loop {
        if !visited {
            black_box(cursor.node().kind());
            count += 1;
        }
        if (!visited && cursor.goto_first_child()) || cursor.goto_next_sibling() {
            visited = false;
        } else if cursor.goto_parent() {
            visited = true;
            if cursor.node().id() == root.id() {
                break;
            }
        } else {
            break;
        }
    }

    count
}

/// Parse every fixture once and verify the tree is clean, so error-recovery
/// (which is slower and unrepresentative) can never leak into a timing.
fn prepare() -> Vec<Case> {
    fixtures::sources()
        .iter()
        .map(|fixture| {
            let language = grammar(fixture);
            let content = fixtures::repeat_to_lines(fixture.content, LINES);
            let tree = with_parsed_tree(language.clone(), &content, |t| t)
                .unwrap_or_else(|| panic!("`{}` failed to parse", fixture.name));

            assert!(
                !tree.root_node().has_error(),
                "scaled `{}` fixture parses with syntax errors; repetition must stay valid",
                fixture.name
            );
            assert!(
                walk_floor(tree.root_node()) > 1,
                "`{}` produced an empty tree",
                fixture.name
            );

            Case {
                name: fixture.name,
                language,
                content,
                tree,
            }
        })
        .collect()
}

pub fn bench(c: &mut Criterion) {
    let cases = prepare();

    let mut parse = c.benchmark_group("parse/parse");
    parse.sample_size(20);
    for case in &cases {
        parse.throughput(Throughput::Bytes(case.content.len() as u64));
        parse.bench_function(BenchmarkId::from_parameter(case.name), |b| {
            b.iter(|| with_parsed_tree(case.language.clone(), black_box(&case.content), |t| t));
        });
    }
    parse.finish();

    let mut complexity = c.benchmark_group("parse/complexity_walk");
    for case in &cases {
        let nodes = walk_floor(case.tree.root_node());
        complexity.throughput(Throughput::Elements(nodes as u64));
        complexity.bench_function(BenchmarkId::from_parameter(case.name), |b| {
            b.iter(|| ast_complexity(black_box(case.tree.root_node()), case.content.as_bytes()));
        });
    }
    complexity.finish();

    let mut floor = c.benchmark_group("parse/walk_floor");
    for case in &cases {
        let nodes = walk_floor(case.tree.root_node());
        floor.throughput(Throughput::Elements(nodes as u64));
        floor.bench_function(BenchmarkId::from_parameter(case.name), |b| {
            b.iter(|| walk_floor(black_box(case.tree.root_node())));
        });
    }
    floor.finish();
}
