use maca_parser::ast::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The call that reads a file while the program is being built.
const DATA: &str = "data";

/// The call that binds a name to whatever the browser has saved under a key.
const STORED: &str = "stored";

/// `std/json`'s reader, which `data` is written in terms of.
const DECODE: &str = "decode";

/// `web/storage`'s reader, which `stored` is written in terms of.
const START: &str = "local_start";

/// `web/storage`'s writer, which an assignment to a stored name is written in terms of.
const STORE: &str = "local_store";

/// Rewrite the two forms a program writes for its host, answering the bytes that were read so a build cache can see them change.
pub fn desugar(m: &mut Module, src: &Path) -> Result<String, String> {
    let defined = defined_names(m);
    let consts = string_constants(m);
    let mut witness = String::new();
    if !defined.contains(DATA) {
        embed_files(m, src, &consts, &defined, &mut witness)?;
    }
    if !defined.contains(STORED) {
        bind_stored(m, &consts, &defined)?;
    }
    Ok(witness)
}

/// Every top-level name the program defines for itself.
fn defined_names(m: &Module) -> HashSet<String> {
    let mut out = HashSet::new();
    for it in &m.items {
        match it {
            Stmt::Fn(f) => {
                out.insert(f.name.clone());
            }
            Stmt::Alias { name, .. } => {
                out.insert(name.clone());
            }
            Stmt::Bind(b) => {
                if let Expr::Ident(n) = &b.target {
                    out.insert(n.clone());
                }
            }
            _ => {}
        }
    }
    out
}

/// The top-level names bound to a plain string, which is what lets a path be written once and used by name.
fn string_constants(m: &Module) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for it in &m.items {
        if let Stmt::Bind(b) = it
            && let Expr::Ident(n) = &b.target
            && let Some(t) = plain_text(&b.value)
        {
            out.insert(n.clone(), t);
        }
    }
    out
}

/// The text of a string that interpolates nothing.
fn plain_text(e: &Expr) -> Option<String> {
    match e {
        Expr::Str(parts) => match parts.as_slice() {
            [] => Some(String::new()),
            [StrPart::Text(t)] => Some(t.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// The literal string an argument names, written out or bound to a constant.
fn literal_arg(e: &Expr, consts: &HashMap<String, String>) -> Option<String> {
    match e {
        Expr::Ident(n) => consts.get(n).cloned(),
        other => plain_text(other),
    }
}

/// The one positional argument of a call to `name`, if that is what this expression is.
fn call_of<'a>(e: &'a Expr, name: &str) -> Option<&'a [Arg]> {
    match e {
        Expr::Call { callee, args } if **callee == Expr::Ident(name.to_string()) => Some(args),
        _ => None,
    }
}

/// A positional argument's expression.
fn positional(a: &Arg) -> Option<&Expr> {
    match a {
        Arg::Pos(e) => Some(e),
        _ => None,
    }
}

/// A string literal as an expression.
fn text_expr(t: &str) -> Expr {
    Expr::Str(vec![StrPart::Text(t.to_string())])
}

/// A call to one of the names a desugaring is written in terms of.
fn call_to(name: &str, args: Vec<Expr>) -> Expr {
    Expr::Call {
        callee: Box::new(Expr::Ident(name.to_string())),
        args: args.into_iter().map(Arg::Pos).collect(),
    }
}

/// The file `data("x")` reads: the `.local` companion when the tree holds one, and otherwise the path as written.
pub fn data_file(base: &Path, spec: &str) -> PathBuf {
    let local = base.join(local_spec(spec));
    if local.is_file() {
        return local;
    }
    base.join(spec)
}

/// `config/links.json` → `config/links.local.json`: the private copy that shadows the committed one.
fn local_spec(spec: &str) -> String {
    let cut = spec.rfind('/').map(|i| i + 1).unwrap_or(0);
    match spec[cut..].rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => {
            format!("{}{stem}.local.{ext}", &spec[..cut])
        }
        _ => format!("{spec}.local"),
    }
}

/// Replace every `data("f")` with the file's text, read now, and read into the type the binding declares.
fn embed_files(
    m: &mut Module,
    src: &Path,
    consts: &HashMap<String, String>,
    defined: &HashSet<String>,
    witness: &mut String,
) -> Result<(), String> {
    let base = src.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut read = 0usize;
    let mut visit = |e: &mut Expr| -> Result<(), String> {
        let Some(args) = call_of(e, DATA) else {
            return Ok(());
        };
        let spec = match args {
            [one] => positional(one)
                .and_then(|a| literal_arg(a, consts))
                .ok_or_else(|| {
                    format!("{DATA}(…): the path is read while building, so write it out or bind it to a constant")
                })?,
            _ => return Err(format!("{DATA}(…) takes one path, as in `{DATA}(\"config/links.json\")`")),
        };
        let path = data_file(&base, &spec);
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("{DATA}(\"{spec}\"): {}: {e}", path.display()))?;
        let text = String::from_utf8(bytes)
            .map_err(|e| format!("{DATA}(\"{spec}\"): not UTF-8 text: {e}"))?;
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        witness.push_str(&path.to_string_lossy());
        witness.push('\n');
        witness.push_str(&text);
        read += 1;
        *e = call_to(DECODE, vec![text_expr(&text)]);
        Ok(())
    };
    walk_module(m, &mut visit)?;
    if read > 0 && !defined.contains(DECODE) {
        return Err(format!(
            "{DATA}(…) reads the file into the type the binding declares; \
             add `import {{ {DECODE} }} from std/json`"
        ));
    }
    Ok(())
}

/// Bind each `stored(key, default)` to what the browser has, and make every later write to that name a save.
fn bind_stored(
    m: &mut Module,
    consts: &HashMap<String, String>,
    defined: &HashSet<String>,
) -> Result<(), String> {
    let mut slots: Vec<(String, String)> = Vec::new();
    for it in &mut m.items {
        let Stmt::Bind(b) = it else { continue };
        let Expr::Ident(name) = b.target.clone() else {
            continue;
        };
        let Some(args) = call_of(&b.value, STORED) else {
            continue;
        };
        let [key, default] = args else {
            return Err(format!(
                "{STORED}(…) takes a key and the value to start from, as in \
                 `{STORED}(\"homepage.locked\", true)`"
            ));
        };
        let (Some(key), Some(default)) = (positional(key), positional(default)) else {
            return Err(format!("{STORED}(…) takes two positional arguments"));
        };
        let literal = literal_arg(key, consts).ok_or_else(|| {
            format!("{STORED}(…): the key names a slot in the browser, so write it out or bind it to a constant")
        })?;
        if b.is_const {
            return Err(format!(
                "`{name}` is a constant, and a stored name is written back whenever it is assigned"
            ));
        }
        if slots.iter().any(|(n, _)| n == &name) {
            return Err(format!("`{name}` is stored twice"));
        }
        b.value = default.clone();
        slots.push((name, literal));
    }
    if slots.is_empty() {
        return Ok(());
    }
    if !defined.contains(START) || !defined.contains(STORE) {
        return Err(format!(
            "{STORED}(…) keeps its value in the browser; add `import web/storage`"
        ));
    }
    let keys: HashMap<String, String> = slots.iter().cloned().collect();
    save_on_assignment(m, &keys);
    m.items.push(Stmt::Import(Import::Foreign {
        lang: "js".to_string(),
        spec: restore_block(&slots),
    }));
    Ok(())
}

/// The bridge call that reads each stored name back before the page is built for the first time, its declared value standing in as the default.
fn restore_block(slots: &[(String, String)]) -> String {
    let mut out = String::from("\n");
    for (name, key) in slots {
        out.push_str(&format!(
            "maca.set({name:?}, {START}({key:?}, maca.get({name:?})));\n"
        ));
    }
    out
}

/// Turn every write to a stored name into the save that answers it, so assignment is the whole of what a program writes.
fn save_on_assignment(m: &mut Module, keys: &HashMap<String, String>) {
    let mut shadowed: Vec<String> = Vec::new();
    for it in &mut m.items {
        if declares_a_slot(it, keys) {
            continue;
        }
        save_in_stmt(it, keys, &mut shadowed);
    }
}

/// Is this the top-level binding that named the slot, whose value is the default rather than a write?
fn declares_a_slot(s: &Stmt, keys: &HashMap<String, String>) -> bool {
    matches!(s, Stmt::Bind(b) if matches!(&b.target, Expr::Ident(n) if keys.contains_key(n)))
}

fn saved_value(name: &str, value: &Expr, keys: &HashMap<String, String>) -> Option<Expr> {
    let key = keys.get(name)?;
    if call_of(value, START).is_some() || call_of(value, STORE).is_some() {
        return None;
    }
    Some(call_to(STORE, vec![text_expr(key), value.clone()]))
}

fn save_in_stmt(s: &mut Stmt, keys: &HashMap<String, String>, shadowed: &mut Vec<String>) {
    match s {
        Stmt::Bind(b) => {
            save_in_expr(&mut b.value, keys, shadowed);
            if let Expr::Ident(n) = &b.target
                && !shadowed.contains(n)
                && let Some(v) = saved_value(n, &b.value, keys)
            {
                b.value = v;
            }
        }
        Stmt::Alias { value, .. } => save_in_expr(value, keys, shadowed),
        Stmt::Expr(e) => save_in_expr(e, keys, shadowed),
        Stmt::Fn(f) => {
            let depth = shadowed.len();
            shadowed.extend(f.params.iter().map(|p| p.name.clone()));
            match &mut f.body {
                Some(FnBody::Block(items)) => {
                    for it in items {
                        save_in_stmt(it, keys, shadowed);
                    }
                }
                Some(FnBody::Expr(e)) => save_in_expr(e, keys, shadowed),
                None => {}
            }
            shadowed.truncate(depth);
        }
        Stmt::Import(_) => {}
    }
}

fn save_in_expr(e: &mut Expr, keys: &HashMap<String, String>, shadowed: &mut Vec<String>) {
    if let Expr::Lambda { params, body, .. } = e {
        let depth = shadowed.len();
        shadowed.extend(params.iter().map(|p| p.name.clone()));
        save_in_expr(body, keys, shadowed);
        shadowed.truncate(depth);
        return;
    }
    if let Expr::For { pat, iter, body } = e {
        save_in_expr(iter, keys, shadowed);
        let depth = shadowed.len();
        shadowed.extend(pattern_names(pat));
        for it in body {
            save_in_stmt(it, keys, shadowed);
        }
        shadowed.truncate(depth);
        return;
    }
    children_mut(e, &mut |c| save_in_expr(c, keys, shadowed));
    stmt_children_mut(e, &mut |c| save_in_stmt(c, keys, shadowed));
    if let Expr::Assign { target, value } = e
        && let Expr::Ident(n) = &**target
        && !shadowed.contains(n)
        && let Some(v) = saved_value(n, value, keys)
    {
        **value = v;
    }
}

/// Every name a `for` pattern binds, which is a name the loop body's writes are about the loop's own value rather than the program's.
fn pattern_names(p: &Pattern) -> Vec<String> {
    match p {
        Pattern::Bind(n) => vec![n.clone()],
        Pattern::Ctor { args, .. } | Pattern::Or(args) => {
            args.iter().flat_map(pattern_names).collect()
        }
        Pattern::List { elems, rest } => elems
            .iter()
            .chain(rest.iter().map(|b| &**b))
            .flat_map(pattern_names)
            .collect(),
        Pattern::Record(fields) => fields
            .iter()
            .flat_map(|(n, p)| match p {
                Some(p) => pattern_names(p),
                None => vec![n.clone()],
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Every expression in the module, children before the expression that holds them.
fn walk_module(
    m: &mut Module,
    f: &mut impl FnMut(&mut Expr) -> Result<(), String>,
) -> Result<(), String> {
    for it in &mut m.items {
        walk_stmt(it, f)?;
    }
    Ok(())
}

fn walk_stmt(
    s: &mut Stmt,
    f: &mut impl FnMut(&mut Expr) -> Result<(), String>,
) -> Result<(), String> {
    match s {
        Stmt::Bind(b) => walk_expr(&mut b.value, f),
        Stmt::Alias { value, .. } => walk_expr(value, f),
        Stmt::Expr(e) => walk_expr(e, f),
        Stmt::Fn(d) => match &mut d.body {
            Some(FnBody::Block(items)) => {
                for it in items {
                    walk_stmt(it, f)?;
                }
                Ok(())
            }
            Some(FnBody::Expr(e)) => walk_expr(e, f),
            None => Ok(()),
        },
        Stmt::Import(_) => Ok(()),
    }
}

fn walk_expr(
    e: &mut Expr,
    f: &mut impl FnMut(&mut Expr) -> Result<(), String>,
) -> Result<(), String> {
    let mut err = None;
    children_mut(e, &mut |c| {
        if err.is_none()
            && let Err(msg) = walk_expr(c, f)
        {
            err = Some(msg);
        }
    });
    stmt_children_mut(e, &mut |c| {
        if err.is_none()
            && let Err(msg) = walk_stmt(c, f)
        {
            err = Some(msg);
        }
    });
    match err {
        Some(msg) => Err(msg),
        None => f(e),
    }
}

/// Every expression one expression holds directly.
fn children_mut(e: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    match e {
        Expr::Str(parts) => {
            for p in parts {
                if let StrPart::Interp(x) = p {
                    f(x);
                }
            }
        }
        Expr::List(xs) => xs.iter_mut().for_each(f),
        Expr::Record(fields) | Expr::Ctor { fields, .. } => field_exprs(fields, f),
        Expr::With { base, fields } => {
            f(base);
            field_exprs(fields, f);
        }
        Expr::Call { callee, args } => {
            f(callee);
            for a in args {
                match a {
                    Arg::Pos(x) => f(x),
                    Arg::Named { value, .. } | Arg::Directive { value, .. } => f(value),
                }
            }
        }
        Expr::Field { base, .. } => f(base),
        Expr::Index { base, index } => {
            f(base);
            f(index);
        }
        Expr::Range { lo, hi } => {
            f(lo);
            f(hi);
        }
        Expr::Unary { expr, .. } => f(expr),
        Expr::Binary { lhs, rhs, .. } => {
            f(lhs);
            f(rhs);
        }
        Expr::Ternary { cond, then, els } => {
            f(cond);
            f(then);
            f(els);
        }
        Expr::If { cond, .. } | Expr::While { cond, .. } => f(cond),
        Expr::Match { scrut, arms } => {
            f(scrut);
            for a in arms {
                if let Some(g) = &mut a.guard {
                    f(g);
                }
                f(&mut a.body);
            }
        }
        Expr::For { iter, .. } => f(iter),
        Expr::Return(Some(x)) => f(x),
        Expr::Lambda { body, .. } => f(body),
        Expr::Try(x) | Expr::Fail(x) | Expr::Reify(x) | Expr::Await(x) | Expr::Spawn(x) => f(x),
        Expr::Assign { target, value } => {
            f(target);
            f(value);
        }
        _ => {}
    }
}

fn field_exprs(fields: &mut [Field], f: &mut impl FnMut(&mut Expr)) {
    for fd in fields {
        match fd {
            Field::Value { value, .. } => f(value),
            Field::Bare(x) => f(x),
            _ => {}
        }
    }
}

/// Every statement one expression holds directly.
fn stmt_children_mut(e: &mut Expr, f: &mut impl FnMut(&mut Stmt)) {
    match e {
        Expr::Block(items) | Expr::For { body: items, .. } | Expr::While { body: items, .. } => {
            items.iter_mut().for_each(f)
        }
        Expr::If { then, els, .. } => {
            then.iter_mut().for_each(&mut *f);
            if let Some(els) = els {
                els.iter_mut().for_each(f);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_private_copy_is_the_committed_name_with_local_before_the_extension() {
        assert_eq!(local_spec("config/links.json"), "config/links.local.json");
        assert_eq!(local_spec("links"), "links.local");
        assert_eq!(local_spec("a.b/links"), "a.b/links.local");
        assert_eq!(local_spec("a.b/links.json"), "a.b/links.local.json");
        assert_eq!(local_spec(".hidden"), ".hidden.local");
    }
}
