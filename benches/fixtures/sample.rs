// Author: kelexine <https://github.com/kelexine>
// Benchmark fixture: representative Rust source (comments, nesting, strings).

/* Block comment
   /* nested block comment */
   still inside the outer comment */

use std::collections::HashMap;

/// A tiny inventory used purely as benchmark input.
#[derive(Debug, Default)]
pub struct Inventory {
    items: HashMap<String, u32>,
}

impl Inventory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `count` units of `name`, returning the new total.
    pub fn add(&mut self, name: &str, count: u32) -> u32 {
        let slot = self.items.entry(name.to_string()).or_insert(0);
        *slot += count;
        *slot
    }

    pub fn remove(&mut self, name: &str, count: u32) -> Result<u32, String> {
        match self.items.get_mut(name) {
            Some(slot) if *slot >= count => {
                *slot -= count;
                Ok(*slot)
            }
            Some(_) => Err(format!("insufficient stock for {name}")),
            None => Err(String::from("unknown item // not a comment")),
        }
    }

    pub fn report(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (name, count) in &self.items {
            if *count == 0 || name.is_empty() {
                continue;
            }
            lines.push(format!("{name}: {count} /* not a comment */"));
        }
        lines.sort();
        lines
    }
}

fn classify(total: u32) -> &'static str {
    if total > 100 && total % 2 == 0 {
        "bulk-even"
    } else if total > 100 || total == 0 {
        "bulk-or-empty"
    } else {
        "regular"
    }
}

pub fn summarize(inv: &Inventory) -> String {
    let mut out = String::new();
    for line in inv.report() {
        let n: u32 = line
            .rsplit(": ")
            .next()
            .and_then(|s| s.split_whitespace().next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        out.push_str(classify(n));
        out.push('\n');
    }
    out
}
