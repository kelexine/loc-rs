// Author: kelexine (https://github.com/kelexine)
// extractors/rust.rs — Rust function/struct extraction via Tree-sitter

use super::Extractor;
use super::tree_sitter::ComplexityIndex;
use crate::models::FunctionInfo;
use tree_sitter::Node;

pub struct RustExtractor;

impl Extractor for RustExtractor {
    fn extract(&self, content: &str) -> Vec<FunctionInfo> {
        super::with_parsed_tree(tree_sitter_rust::LANGUAGE.into(), content, |tree| {
            let index = ComplexityIndex::build(tree.root_node(), content.as_bytes());
            let mut functions = Vec::new();
            traverse(tree.root_node(), content, &mut functions, false, &index);
            functions.sort_by_key(|f| f.line_start);
            functions
        })
        .unwrap_or_default()
    }
}

/// One unit of work for the traversal below.
enum Step<'a> {
    /// A function/struct already parsed; just needs appending in this order.
    Emit(FunctionInfo),
    /// A node still to be visited.
    Visit(Node<'a>, bool),
}

/// Depth-first traversal of the AST, collecting functions, structs and methods.
///
/// This is an **explicit-stack** DFS rather than a recursive walk. Extraction
/// runs on rayon worker threads, whose stacks are 2 MiB against 8 MiB on the
/// main thread, so unbounded recursion over deeply nested input can abort the
/// whole process.
///
/// Ordering matters and is not the obvious one. Unlike the other extractors,
/// this walk does **not** descend into `function_item`/`struct_item` children of
/// a container — it parses them and moves on. A recursive walk therefore emits
/// them *interleaved* with descendant emissions: for `impl A { fn inner() {} }
/// fn free_fn() {}` it pushes `inner` (from the impl subtree) before
/// `free_fn`, even though `free_fn` is a later sibling of the impl. So each
/// container builds an ordered action list, which is pushed in reverse and so
/// consumed in the original child order. `extract` then sorts by `line_start`
/// with a *stable* sort, which makes this visit order observable.
fn traverse(
    node: Node,
    content: &str,
    functions: &mut Vec<FunctionInfo>,
    in_impl: bool,
    index: &ComplexityIndex,
) {
    let mut stack: Vec<Step<'_>> = vec![Step::Visit(node, in_impl)];
    // Reused across nodes so the traversal does not allocate per node.
    let mut children: Vec<Node> = Vec::new();
    let mut actions: Vec<Step<'_>> = Vec::new();

    while let Some(step) = stack.pop() {
        let (node, in_impl) = match step {
            Step::Emit(info) => {
                functions.push(info);
                continue;
            }
            Step::Visit(n, f) => (n, f),
        };

        let kind = node.kind();
        let is_impl = kind == "impl_item";

        // Collect outer attributes that precede a function/struct item.
        // In tree-sitter-rust outer attribute_item nodes are siblings, not children.
        // We walk children of the current node and carry pending attributes forward.
        if kind == "source_file" || kind == "impl_item" || kind == "block" {
            children.clear();
            {
                let mut cursor = node.walk();
                children.extend(node.children(&mut cursor));
            }

            actions.clear();
            let mut pending_attrs: Vec<String> = Vec::new();
            for &child in &children {
                let ckind = child.kind();
                if ckind == "attribute_item" {
                    let text = child.utf8_text(content.as_bytes()).unwrap_or("");
                    pending_attrs.push(text.to_string());
                } else if ckind == "function_item" {
                    let is_test = pending_attrs.iter().any(|a| a.contains("test"));
                    pending_attrs.clear();
                    if !is_test
                        && let Some(info) =
                            parse_function(child, content, in_impl || is_impl, index)
                    {
                        actions.push(Step::Emit(info));
                    }
                } else if ckind == "struct_item" {
                    pending_attrs.clear();
                    if let Some(info) = parse_struct(child, content) {
                        actions.push(Step::Emit(info));
                    }
                } else if ckind == "impl_item" {
                    pending_attrs.clear();
                    // Descend into impl blocks with in_impl=true
                    actions.push(Step::Visit(child, true));
                } else {
                    pending_attrs.clear();
                    actions.push(Step::Visit(child, in_impl || is_impl));
                }
            }
            stack.extend(actions.drain(..).rev());
            continue;
        }

        if kind == "function_item"
            && let Some(info) = parse_function(node, content, in_impl, index)
        {
            functions.push(info);
        } else if kind == "struct_item"
            && let Some(info) = parse_struct(node, content)
        {
            functions.push(info);
        }

        children.clear();
        {
            let mut cursor = node.walk();
            children.extend(node.children(&mut cursor));
        }
        for &child in children.iter().rev() {
            stack.push(Step::Visit(child, in_impl || is_impl));
        }
    }
}

fn parse_function(
    node: Node,
    content: &str,
    is_method: bool,
    index: &ComplexityIndex,
) -> Option<FunctionInfo> {
    let mut name = String::new();
    let mut is_async = false;
    let mut is_pub = false;
    let mut params_str = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind == "identifier" && name.is_empty() {
            name = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
        } else if kind == "function_modifiers" {
            // In tree-sitter-rust, `async` lives inside function_modifiers
            let mod_text = child.utf8_text(content.as_bytes()).unwrap_or("");
            if mod_text.contains("async") {
                is_async = true;
            }
        } else if kind == "visibility_modifier" {
            is_pub = true;
        } else if kind == "parameters" {
            params_str = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
        }
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    let complexity = index.get(node);

    let mut parameters = Vec::new();
    let trimmed_params = params_str.trim_start_matches('(').trim_end_matches(')');
    if !trimmed_params.is_empty() {
        for p in trimmed_params.split(',') {
            let p_trim = p.trim();
            if !p_trim.is_empty() {
                parameters.push(p_trim.to_string());
            }
        }
    }

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters,
        is_async,
        is_method,
        is_class: false,
        docstring: None,
        decorators: if is_pub { vec!["pub".into()] } else { vec![] },
        complexity,
    })
}

fn parse_struct(node: Node, content: &str) -> Option<FunctionInfo> {
    let mut name = String::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_identifier" {
            name = child
                .utf8_text(content.as_bytes())
                .unwrap_or("")
                .to_string();
            break;
        }
    }

    if name.is_empty() {
        name = "?".to_string();
    }

    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    Some(FunctionInfo {
        name,
        line_start: start_line,
        line_end: end_line,
        parameters: vec![],
        is_async: false,
        is_method: false,
        is_class: true,
        docstring: None,
        decorators: vec![],
        complexity: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_functions() {
        let content = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }

async fn fetch() {}

struct Point { x: f64, y: f64 }

impl Point {
    pub fn new(x: f64, y: f64) -> Self { Point { x, y } }
}
"#;
        let extractor = RustExtractor;
        let mut fns = extractor.extract(content);
        fns.sort_by(|a, b| a.name.cmp(&b.name));

        // add, fetch, new, Point (struct)
        assert_eq!(fns.len(), 4);

        let add = fns.iter().find(|f| f.name == "add").unwrap();
        assert!(!add.is_async);
        assert!(add.decorators.contains(&"pub".to_string()));

        let fetch = fns.iter().find(|f| f.name == "fetch").unwrap();
        assert!(fetch.is_async);

        let point = fns.iter().find(|f| f.name == "Point").unwrap();
        assert!(point.is_class);

        let new = fns.iter().find(|f| f.name == "new").unwrap();
        assert!(new.is_method);
    }

    #[test]
    fn test_rust_test_functions_excluded() {
        let content = r#"
#[test]
fn my_test() {}

fn real_fn() {}
"#;
        let extractor = RustExtractor;
        let fns = extractor.extract(content);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "real_fn");
    }

    // ── Visit-order regression (explicit-stack DFS) ─────────────────────────
    //
    // `extract` sorts by `line_start` with a *stable* sort, so when two
    // functions share a start line the traversal order decides the output
    // order. The explicit-stack walk must therefore visit siblings in the
    // same pre-order a recursive walk would.

    #[test]
    fn test_visit_order_multiple_impls_same_line() {
        // Both impls (and both methods) start on line 1. Pre-order visit is
        // A then B, so `a` must precede `b` in the output.
        let content = "impl A { fn a() {} }\nimpl B { fn b() {} }\n";
        let fns = RustExtractor.extract(content);
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"], "pre-order visit must be preserved");
        assert!(fns.iter().all(|f| f.is_method));
    }

    #[test]
    fn test_visit_order_impl_before_free_function() {
        // The impl is descended into *before* the following sibling is handled,
        // so `inner` is emitted first even though the free function is the
        // later sibling. Both start on line 1, so the stable sort by
        // `line_start` is a no-op and traversal order is the only variable.
        let content = "impl A { fn inner() {} } fn free_fn() {}\n";
        let fns = RustExtractor.extract(content);
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["inner", "free_fn"]);
        assert!(fns.iter().find(|f| f.name == "inner").unwrap().is_method);
        assert!(!fns.iter().find(|f| f.name == "free_fn").unwrap().is_method);
    }

    #[test]
    fn test_visit_order_sorts_by_line_when_lines_differ() {
        // Same input as above but on separate lines: `inner` starts on line 1,
        // so the sort must place it first regardless of visit order.
        let content = "impl A { fn inner() {} }\nfn free_fn() {}\n";
        let fns = RustExtractor.extract(content);
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["inner", "free_fn"]);
    }

    #[test]
    fn test_visit_order_sibling_functions_same_line() {
        // Two functions on one line: parsed inline in source order.
        let content = "fn a() { let x = 1; } fn b() { let y = 2; }\n";
        let fns = RustExtractor.extract(content);
        let names: Vec<&str> = fns.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn test_deep_nesting_does_not_overflow_stack() {
        // Regression: the recursive walk overflowed a 2 MiB thread (rayon
        // worker default) at roughly 20k levels of AST depth. The
        // explicit-stack walk must handle this without recursing.
        let depth = 40_000;
        let content = format!(
            "fn deep() {{ let a = 1; {}{} let b = 2; }}\n",
            "{ ".repeat(depth),
            "}".repeat(depth)
        );
        let fns = RustExtractor.extract(&content);
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "deep");
    }

    // ── Differential: explicit-stack vs the original recursive walk ────────

    /// The pre-refactor recursive walk, kept verbatim as a reference oracle.
    /// If this and `traverse` ever disagree, the refactor changed output.
    #[allow(clippy::only_used_in_recursion)]
    fn reference_traverse(
        node: Node,
        content: &str,
        functions: &mut Vec<FunctionInfo>,
        in_impl: bool,
        index: &ComplexityIndex,
    ) {
        let kind = node.kind();
        let is_impl = kind == "impl_item";

        if kind == "source_file" || kind == "impl_item" || kind == "block" {
            let mut pending_attrs: Vec<String> = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let ckind = child.kind();
                if ckind == "attribute_item" {
                    let text = child.utf8_text(content.as_bytes()).unwrap_or("");
                    pending_attrs.push(text.to_string());
                } else if ckind == "function_item" {
                    let is_test = pending_attrs.iter().any(|a| a.contains("test"));
                    pending_attrs.clear();
                    if !is_test
                        && let Some(info) =
                            parse_function(child, content, in_impl || is_impl, index)
                    {
                        functions.push(info);
                    }
                } else if ckind == "struct_item" {
                    pending_attrs.clear();
                    if let Some(info) = parse_struct(child, content) {
                        functions.push(info);
                    }
                } else if ckind == "impl_item" {
                    pending_attrs.clear();
                    reference_traverse(child, content, functions, true, index);
                } else {
                    pending_attrs.clear();
                    reference_traverse(child, content, functions, in_impl || is_impl, index);
                }
            }
            return;
        }

        if kind == "function_item"
            && let Some(info) = parse_function(node, content, in_impl, index)
        {
            functions.push(info);
        } else if kind == "struct_item"
            && let Some(info) = parse_struct(node, content)
        {
            functions.push(info);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            reference_traverse(child, content, functions, in_impl || is_impl, index);
        }
    }

    /// Run the reference oracle exactly as `RustExtractor::extract` does.
    fn reference_extract(content: &str) -> Vec<FunctionInfo> {
        crate::extractors::with_parsed_tree(tree_sitter_rust::LANGUAGE.into(), content, |tree| {
            let index = ComplexityIndex::build(tree.root_node(), content.as_bytes());
            let mut functions = Vec::new();
            reference_traverse(tree.root_node(), content, &mut functions, false, &index);
            functions.sort_by_key(|f| f.line_start);
            functions
        })
        .unwrap_or_default()
    }

    /// Sources chosen to stress the two orderings that are easy to break:
    /// several items sharing a start line (where the stable sort by
    /// `line_start` exposes visit order), impl blocks that are deferred to the
    /// descend stack, and `#[test]`-attributed functions.
    const DIFFERENTIAL_SOURCES: &[&str] = &[
        "impl A { fn m1() {} }\nimpl B { fn m2() {} }\nstruct S1; struct S2;\nfn free1() {} fn free2() {}\n",
        "impl A { fn inner() {} } fn free_fn() {}\n",
        "impl A { fn inner() {} }\nfn free_fn() {}\n",
        "fn a() { let x = 1; } fn b() { let y = 2; }\n",
        "mod m1 { fn f() {} }\nmod m2 { fn g() {} }\nmod m3 { pub fn h() {} }\n",
        "#[cfg(test)]\nmod t { #[test] fn hidden() {} }\n#[test]\nfn also_hidden() {}\nfn visible() {}\n",
        "trait Tr { fn required(&self); }\nimpl Tr for A { fn required(&self) {} }\nimpl A { fn helper() {} }\n",
        "struct A;\nstruct B;\nimpl A { fn m() {} }\nimpl B { fn n() {} }\nstruct C;\n",
        "fn a() {}\nstruct S;\nimpl S { fn m() {} }\nfn b() {}\nmod m { fn c() {} }\n",
        "impl A { fn m() {} struct Inner; }\nimpl B { fn n() {} struct Other; }\n",
        "",
        "   \n\n",
    ];

    #[test]
    fn test_explicit_stack_matches_recursive_reference() {
        for (i, src) in DIFFERENTIAL_SOURCES.iter().enumerate() {
            let reference = reference_extract(src);
            let actual = RustExtractor.extract(src);
            assert_eq!(
                reference.len(),
                actual.len(),
                "source #{i}: function count differs\n{:#?}\nvs\n{:#?}",
                reference,
                actual
            );
            for (r, a) in reference.iter().zip(actual.iter()) {
                assert_eq!(r.name, a.name, "source #{i}: name differs");
                assert_eq!(
                    r.line_start, a.line_start,
                    "source #{i}: line_start differs"
                );
                assert_eq!(r.line_end, a.line_end, "source #{i}: line_end differs");
                assert_eq!(
                    r.complexity, a.complexity,
                    "source #{i}: complexity differs for {}",
                    r.name
                );
                assert_eq!(r.is_method, a.is_method, "source #{i}: is_method");
                assert_eq!(r.is_class, a.is_class, "source #{i}: is_class");
                assert_eq!(r.parameters, a.parameters, "source #{i}: parameters");
                assert_eq!(r.decorators, a.decorators, "source #{i}: decorators");
                assert_eq!(r.is_async, a.is_async, "source #{i}: is_async");
            }
        }
    }

    #[test]
    fn test_differential_sources_are_not_vacuous() {
        // Guard against the oracle silently returning empty for everything,
        // which would make the differential test pass trivially.
        let total: usize = DIFFERENTIAL_SOURCES
            .iter()
            .map(|s| reference_extract(s).len())
            .sum();
        assert!(
            total >= 20,
            "differential corpus too weak: only {total} functions extracted"
        );
    }
}
