//! Types, unification, and effect sets.
//!
//! Unification is deliberately *gradual*: `Any` unifies with anything and never
//! produces an error. That is the escape hatch that lets the checker stay
//! optimistic about unknown stdlib/foreign values (their boundary type is
//! `any`) while still catching a genuine mismatch between two concrete types.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Int,
    Float,
    Str,
    Bool,
    Bytes,
    Unit,
    /// Named nominal type with optional args: `Status`, `Task`, `Array T`,
    /// `Map k v`, sized numerics (`i32`), SIMD vectors (`f32x8`), `Element`…
    Con(String, Vec<Ty>),
    Fn(Vec<Ty>, Box<Ty>),
    /// Structural record. `open` records tolerate extra/missing fields (row
    /// polymorphism, lightweight): field access always yields an open record.
    Rec {
        fields: BTreeMap<String, Ty>,
        open: bool,
    },
    Opt(Box<Ty>),
    Var(u32),
    Any,
}

impl Ty {
    pub fn array(t: Ty) -> Ty {
        Ty::Con("Array".into(), vec![t])
    }
}

/// A (rank-1) polymorphic type scheme: `ty` with `vars` universally quantified.
/// Function signatures are generalized into schemes at declaration and
/// *instantiated* with fresh variables at each use site — that is what makes a
/// generic like `id(x: a) -> a` usable at `int` and `str` in the same program
/// without the two uses colliding.
#[derive(Clone, Debug)]
pub struct Scheme {
    pub vars: Vec<u32>,
    pub ty: Ty,
}

impl Scheme {
    /// A monomorphic scheme — nothing quantified.
    pub fn mono(ty: Ty) -> Self {
        Scheme {
            vars: Vec::new(),
            ty,
        }
    }
}

/// Is `n` a type *variable* by the language's naming convention? Nominal types
/// are capitalized (`Task`, `Array`), primitives are keywords (`int`, `str`),
/// and sized-numeric / SIMD lane types start with `i`/`u`/`f` + a digit
/// (`i32`, `f32x8`). Everything else lowercase (`a`, `k`, `value`) is a
/// type variable.
///
/// The C and JVM backends monomorphize against the same rule, so it is spelled
/// once, beside the AST all three read.
pub use maca_parser::ast::is_type_var_name;

/// Union-find inference context.
#[derive(Default)]
pub struct Infer {
    subst: Vec<Option<Ty>>,
}

impl Infer {
    pub fn fresh(&mut self) -> Ty {
        let id = self.subst.len() as u32;
        self.subst.push(None);
        Ty::Var(id)
    }

    /// Instantiate a scheme: give each quantified variable a fresh copy so a
    /// polymorphic function can be used at many types independently.
    pub fn instantiate(&mut self, s: &Scheme) -> Ty {
        if s.vars.is_empty() {
            return s.ty.clone();
        }
        let mut map = BTreeMap::new();
        for &v in &s.vars {
            let fresh = self.fresh();
            map.insert(v, fresh);
        }
        subst(&s.ty, &map)
    }

    /// Follow variable bindings one-or-more hops to the representative type.
    pub fn resolve(&self, t: &Ty) -> Ty {
        match t {
            Ty::Var(i) => match &self.subst[*i as usize] {
                Some(u) => self.resolve(u),
                None => Ty::Var(*i),
            },
            _ => t.clone(),
        }
    }

    /// Unify two types. Returns `Err` only on a concrete/concrete clash.
    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), String> {
        let a = self.resolve(a);
        let b = self.resolve(b);
        match (a, b) {
            (Ty::Any, _) | (_, Ty::Any) => Ok(()),
            (Ty::Var(i), Ty::Var(j)) if i == j => Ok(()),
            (Ty::Var(i), t) | (t, Ty::Var(i)) => {
                if occurs(i, &t, self) {
                    self.subst[i as usize] = Some(Ty::Any); // avoid infinite type
                } else {
                    self.subst[i as usize] = Some(t);
                }
                Ok(())
            }
            (Ty::Int, Ty::Int)
            | (Ty::Float, Ty::Float)
            | (Ty::Str, Ty::Str)
            | (Ty::Bool, Ty::Bool)
            | (Ty::Bytes, Ty::Bytes)
            | (Ty::Unit, Ty::Unit) => Ok(()),
            (Ty::Con(n, xs), Ty::Con(m, ys)) if n == m && xs.len() == ys.len() => {
                for (x, y) in xs.iter().zip(&ys) {
                    self.unify(x, y)?;
                }
                Ok(())
            }
            (Ty::Opt(x), Ty::Opt(y)) => self.unify(&x, &y),
            // nullable coercion: `T` unifies with `T?`
            (Ty::Opt(x), y) | (y, Ty::Opt(x)) => self.unify(&x, &y),
            (Ty::Fn(p1, r1), Ty::Fn(p2, r2)) if p1.len() == p2.len() => {
                for (x, y) in p1.iter().zip(&p2) {
                    self.unify(x, y)?;
                }
                self.unify(&r1, &r2)
            }
            (
                Ty::Rec {
                    fields: f1,
                    open: o1,
                },
                Ty::Rec {
                    fields: f2,
                    open: o2,
                },
            ) => {
                for (k, v) in &f1 {
                    match f2.get(k) {
                        Some(w) => self.unify(v, w)?,
                        None if o2 => {}
                        None => return Err(format!("record is missing field `{k}`")),
                    }
                }
                for k in f2.keys() {
                    if !f1.contains_key(k) && !o1 {
                        return Err(format!("record has unexpected field `{k}`"));
                    }
                }
                Ok(())
            }
            (x, y) => Err(format!(
                "type mismatch: expected {}, found {}",
                show(&x),
                show(&y)
            )),
        }
    }
}

/// Substitute variables named in `map` throughout `t` (used by instantiation).
fn subst(t: &Ty, map: &BTreeMap<u32, Ty>) -> Ty {
    match t {
        Ty::Var(i) => map.get(i).cloned().unwrap_or(Ty::Var(*i)),
        Ty::Con(n, args) => Ty::Con(n.clone(), args.iter().map(|a| subst(a, map)).collect()),
        Ty::Fn(ps, r) => Ty::Fn(
            ps.iter().map(|a| subst(a, map)).collect(),
            Box::new(subst(r, map)),
        ),
        Ty::Rec { fields, open } => Ty::Rec {
            fields: fields
                .iter()
                .map(|(k, v)| (k.clone(), subst(v, map)))
                .collect(),
            open: *open,
        },
        Ty::Opt(a) => Ty::Opt(Box::new(subst(a, map))),
        _ => t.clone(),
    }
}

fn occurs(i: u32, t: &Ty, inf: &Infer) -> bool {
    match inf.resolve(t) {
        Ty::Var(j) => i == j,
        Ty::Con(_, args) => args.iter().any(|a| occurs(i, a, inf)),
        Ty::Fn(ps, r) => ps.iter().any(|a| occurs(i, a, inf)) || occurs(i, &r, inf),
        Ty::Rec { fields, .. } => fields.values().any(|a| occurs(i, a, inf)),
        Ty::Opt(a) => occurs(i, &a, inf),
        _ => false,
    }
}

pub fn show(t: &Ty) -> String {
    match t {
        Ty::Int => "int".into(),
        Ty::Float => "float".into(),
        Ty::Str => "str".into(),
        Ty::Bool => "bool".into(),
        Ty::Bytes => "bytes".into(),
        Ty::Unit => "()".into(),
        Ty::Con(n, args) if args.is_empty() => n.clone(),
        Ty::Con(n, args) => {
            format!(
                "{n} {}",
                args.iter().map(show).collect::<Vec<_>>().join(" ")
            )
        }
        Ty::Fn(ps, r) => {
            format!(
                "({}) -> {}",
                ps.iter().map(show).collect::<Vec<_>>().join(", "),
                show(r)
            )
        }
        Ty::Rec { fields, .. } => {
            let fs: Vec<_> = fields
                .iter()
                .map(|(k, v)| format!("{k}: {}", show(v)))
                .collect();
            format!("{{ {} }}", fs.join(", "))
        }
        Ty::Opt(t) => format!("{}?", show(t)),
        Ty::Var(i) => format!("t{i}"),
        Ty::Any => "any".into(),
    }
}

// ---- effects -------------------------------------------------------------

/// Algebraic effect set: `io · net · os · async · exn`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffSet(pub u8);

pub const IO: u8 = 1;
pub const NET: u8 = 2;
pub const OS: u8 = 4;
pub const ASYNC: u8 = 8;
pub const EXN: u8 = 16;

impl EffSet {
    pub fn empty() -> Self {
        EffSet(0)
    }
    pub fn of(flag: u8) -> Self {
        EffSet(flag)
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn union(self, other: EffSet) -> EffSet {
        EffSet(self.0 | other.0)
    }
    pub fn names(self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.0 & IO != 0 {
            v.push("io");
        }
        if self.0 & NET != 0 {
            v.push("net");
        }
        if self.0 & OS != 0 {
            v.push("os");
        }
        if self.0 & ASYNC != 0 {
            v.push("async");
        }
        if self.0 & EXN != 0 {
            v.push("exn");
        }
        v
    }
}
