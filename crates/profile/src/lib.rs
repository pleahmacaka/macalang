//! Shared flame-graph renderer for Maca profiles.
//!
//! `maca profile` records a program's call graph with Callgrind (per-call
//! inclusive instruction reads, `Ir`) and renders a classic flame graph — each
//! frame's width is its share of the cost, depth is call nesting, and
//! self-recursive frames (e.g. `fib`) are collapsed so the chart stays readable.
//!
//! The renderer is factored so the *same* flame graph is produced from any cost
//! model, not just Callgrind: the browser playground drives it from an
//! interpreter's per-function step counts (see `crates/wasm`). Native passes
//! Callgrind `Ir`; the playground passes eval `steps` — same picture, different
//! unit label.

use std::collections::HashMap;

/// One function's cost profile: its own cost plus the inclusive cost of each
/// call it makes (`callee -> inclusive cost`). This is the model both the
/// Callgrind parser and the interpreter populate.
#[derive(Default, Clone)]
pub struct FnCost {
    pub self_cost: u64,
    /// callee -> inclusive cost attributed to calls into it from this function
    pub calls: HashMap<String, u64>,
}

/// Parse a `callgrind.out` file. Returns (per-function costs, total).
///
/// Handles Callgrind name compression: `fn=(12) name` defines id 12 = name,
/// and a later `fn=(12)` refers back to it.
pub fn parse_callgrind(text: &str) -> (HashMap<String, FnCost>, u64) {
    let mut fns: HashMap<String, FnCost> = HashMap::new();
    let mut names: HashMap<String, String> = HashMap::new(); // compression id -> name
    let mut cur: Option<String> = None;
    let mut pending_callee: Option<String> = None;
    let mut total: u64 = 0;

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("fn=") {
            cur = Some(resolve_name(rest, &mut names));
            fns.entry(cur.clone().unwrap()).or_default();
            pending_callee = None;
        } else if let Some(rest) = line.strip_prefix("cfn=") {
            pending_callee = Some(resolve_name(rest, &mut names));
        } else if !line.is_empty()
            && matches!(line.as_bytes()[0], b'0'..=b'9' | b'*' | b'+' | b'-')
            && let Some(f) = &cur
        {
            // cost line: "<pos> <Ir> …". Position may be absolute (`1346`),
            // same (`*`), or relative (`+99` / `-13`); the Ir is the next token.
            let ir = line.split_whitespace().nth(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            if let Some(callee) = pending_callee.take() {
                *fns.get_mut(f).unwrap().calls.entry(callee).or_default() += ir;
            } else {
                fns.get_mut(f).unwrap().self_cost += ir;
                total += ir;
            }
        }
    }
    (fns, total.max(1))
}

/// Resolve a Callgrind name spec (`(id) name`, `(id)`, or bare `name`) against
/// the compression table, recording new `id -> name` mappings.
fn resolve_name(spec: &str, names: &mut HashMap<String, String>) -> String {
    let spec = spec.trim();
    if let Some(rest) = spec.strip_prefix('(') {
        if let Some((id, name)) = rest.split_once(')') {
            let id = id.trim().to_string();
            let name = name.trim();
            if name.is_empty() {
                return names.get(&id).cloned().unwrap_or_default();
            }
            names.insert(id, name.to_string());
            return name.to_string();
        }
    }
    spec.to_string()
}

struct Frame {
    name: String,
    value: u64,
    depth: usize,
    x: u64,
    children: Vec<Frame>,
}

/// Inclusive cost = a function's own cost plus the inclusive cost of every call
/// it makes (call edges already carry the callee's inclusive cost).
pub fn inclusive(name: &str, fns: &HashMap<String, FnCost>) -> u64 {
    let Some(fc) = fns.get(name) else { return 0 };
    fc.self_cost + fc.calls.values().sum::<u64>()
}

fn build(name: &str, value: u64, depth: usize, x: u64, fns: &HashMap<String, FnCost>, path: &mut Vec<String>) -> Frame {
    let mut children = Vec::new();
    if depth < 40 {
        path.push(name.to_string());
        if let Some(fc) = fns.get(name) {
            let mut kids: Vec<(&String, &u64)> = fc.calls.iter().collect();
            kids.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
            let mut cx = x;
            for (callee, ir) in kids {
                if *ir == 0 {
                    continue;
                }
                // collapse recursion: a callee already on the call path would
                // otherwise be drawn with its summed-over-all-frames inclusive
                // cost, which can exceed the parent's width. Skip it.
                if path.contains(callee) {
                    continue;
                }
                // a child can't be wider than its parent's remaining span
                let cv = (*ir).min(value.saturating_sub(cx.saturating_sub(x)));
                if cv == 0 {
                    continue;
                }
                children.push(build(callee, cv, depth + 1, cx, fns, path));
                cx += cv;
            }
        }
        path.pop();
    }
    Frame { name: name.to_string(), value, depth, x, children }
}

fn flatten<'a>(f: &'a Frame, out: &mut Vec<&'a Frame>) {
    out.push(f);
    for c in &f.children {
        flatten(c, out);
    }
}

/// Render a flame graph SVG for a Callgrind dump.
pub fn flamegraph_svg(cg_text: &str) -> String {
    let (fns, _total) = parse_callgrind(cg_text);
    flamegraph_svg_from(&fns, "Ir")
}

/// Render a flame graph SVG from any cost model (`unit` labels the metric,
/// e.g. `Ir` for Callgrind, `steps` for the interpreter). Rooted at `main`, or
/// the costliest function when there is no `main`.
pub fn flamegraph_svg_from(fns: &HashMap<String, FnCost>, unit: &str) -> String {
    let root_name = if fns.contains_key("main") {
        "main".to_string()
    } else {
        fns.iter().max_by_key(|(_, c)| c.self_cost).map(|(n, _)| n.clone()).unwrap_or_default()
    };
    let root_val = inclusive(&root_name, fns).max(1);
    let root = build(&root_name, root_val, 0, 0, fns, &mut Vec::new());

    let mut frames = Vec::new();
    flatten(&root, &mut frames);
    let max_depth = frames.iter().map(|f| f.depth).max().unwrap_or(0);

    let width = 1200.0_f64;
    let row = 20.0_f64;
    let height = (max_depth as f64 + 1.0) * row + 40.0;
    let scale = width / root_val as f64;

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" \
         viewBox=\"0 0 {width:.0} {height:.0}\" \
         font-family=\"ui-monospace, monospace\" font-size=\"11\">\n"
    ));
    svg.push_str(&format!(
        "<rect width=\"{width:.0}\" height=\"{height:.0}\" fill=\"#1b1a24\"/>\n\
         <text x=\"8\" y=\"16\" fill=\"#e6e3f0\" font-size=\"13\">maca flame graph — {} samples ({unit})</text>\n",
        root_val
    ));
    for (i, f) in frames.iter().enumerate() {
        if f.value == 0 {
            continue;
        }
        let x = f.x as f64 * scale;
        let w = (f.value as f64 * scale).max(0.4);
        let y = height - 30.0 - (f.depth as f64 + 1.0) * row;
        let hue = (i as u32 * 47) % 360;
        let label = if w > 42.0 {
            format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#14131b\">{}</text>",
                x + 3.0,
                y + 14.0,
                xml_escape(&elide(&f.name, (w / 6.5) as usize))
            )
        } else {
            String::new()
        };
        svg.push_str(&format!(
            "<g><title>{} — {} {unit} ({:.1}%)</title>\
             <rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" rx=\"1\" \
             fill=\"hsl({hue},55%,62%)\" stroke=\"#1b1a24\" stroke-width=\"0.5\"/>{label}</g>\n",
            xml_escape(&f.name),
            f.value,
            f.value as f64 / root_val as f64 * 100.0,
            x, y, w, row - 1.0,
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max || max < 2 {
        s.to_string()
    } else {
        let keep: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{keep}…")
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A compact text profile (top functions by self cost) for stdout.
pub fn text_profile(cg_text: &str) -> String {
    let (fns, total) = parse_callgrind(cg_text);
    let mut rows: Vec<(&String, u64)> = fns.iter().map(|(n, c)| (n, c.self_cost)).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1));
    let mut out = String::from("  self%     Ir  function\n");
    for (name, ir) in rows.iter().take(12) {
        if *ir == 0 {
            continue;
        }
        out.push_str(&format!("  {:5.1}  {:>9}  {}\n", *ir as f64 / total as f64 * 100.0, ir, name));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal Callgrind dump: main calls fib (100 Ir) and does 5 Ir itself.
    const SAMPLE: &str = "\
events: Ir
fn=(1) main
16 5
cfn=(2) fib
calls=1 16
* 100
fn=(2) fib
20 100
";

    #[test]
    fn parses_compressed_names_and_costs() {
        let (fns, total) = parse_callgrind(SAMPLE);
        assert_eq!(fns["main"].self_cost, 5);
        assert_eq!(fns["main"].calls["fib"], 100);
        assert_eq!(fns["fib"].self_cost, 100);
        assert_eq!(total, 105);
        assert_eq!(inclusive("main", &fns), 105);
    }

    #[test]
    fn flamegraph_is_valid_svg() {
        let svg = flamegraph_svg(SAMPLE);
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("main"));
        assert!(svg.contains("fib"));
    }

    #[test]
    fn flamegraph_from_costs_uses_unit_label() {
        let mut fns = HashMap::new();
        fns.insert("main".to_string(), FnCost { self_cost: 5, calls: HashMap::from([("fib".to_string(), 100)]) });
        fns.insert("fib".to_string(), FnCost { self_cost: 100, calls: HashMap::new() });
        let svg = flamegraph_svg_from(&fns, "steps");
        assert!(svg.contains("samples (steps)"), "{svg}");
        assert!(svg.contains("main") && svg.contains("fib"));
    }
}
