use maca_parser::parse;

fn fndef(src: &str) -> maca_parser::ast::FnDef {
    let m = parse(src).module;
    m.items
        .into_iter()
        .find_map(|it| match it {
            maca_parser::ast::Stmt::Fn(f) => Some(f),
            _ => None,
        })
        .expect("a function")
}

#[test]
fn is_simd_fn_gates_on_vector_types() {
    assert!(maca_backend_llvm::is_simd_fn(&fndef(
        "dot8(a: f32x8, b: f32x8) -> f32 => (a * b).sum()\n"
    )));
    assert!(maca_backend_llvm::is_simd_fn(&fndef(
        "splat(x: f32) -> f32x8 => x\n"
    )));
    assert!(!maca_backend_llvm::is_simd_fn(&fndef(
        "add(a: int, b: int) -> int => a + b\n"
    )));
}

#[test]
fn emits_vector_multiply_and_reduce() {
    let out = maca_backend_llvm::emit(
        &parse("dot8(a: f32x8, b: f32x8) -> f32 => (a * b).sum()\n").module,
    );
    assert!(
        out.ir.contains("<8 x float>"),
        "no vector type:\n{}",
        out.ir
    );
    assert!(out.ir.contains("fmul"), "no vector multiply:\n{}", out.ir);
    assert!(
        out.ir.contains("llvm.vector.reduce.fadd"),
        "no reduce intrinsic:\n{}",
        out.ir
    );
    assert!(
        out.simd_fns.contains(&"dot8".to_string()),
        "not registered: {:?}",
        out.simd_fns
    );
}

#[test]
fn scalar_only_module_emits_no_kernels() {
    let out = maca_backend_llvm::emit(&parse("add(a: int, b: int) -> int => a + b\n").module);
    assert!(
        out.simd_fns.is_empty(),
        "scalar fn should not be a SIMD kernel: {:?}",
        out.simd_fns
    );
}

#[test]
fn block_bodied_kernel_binds_intermediates() {
    let out = maca_backend_llvm::emit(
        &parse("dot(a: f32x8, b: f32x8) -> f32 {\n    p = a * b\n    p.sum()\n}\n").module,
    );
    assert!(
        out.simd_fns.contains(&"dot".to_string()),
        "block-bodied kernel not lowered: {:?}",
        out.simd_fns
    );
    assert!(out.ir.contains("fmul"), "no multiply:\n{}", out.ir);
    assert!(
        out.ir.contains("llvm.vector.reduce.fadd"),
        "no reduce over the bound intermediate:\n{}",
        out.ir
    );
}
