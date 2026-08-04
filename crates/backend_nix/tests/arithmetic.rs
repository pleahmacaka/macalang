fn nix(src: &str) -> String {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    match maca_backend_nix::emit_checked(&p.module) {
        Ok(s) => s,
        Err(e) => panic!("unexpected refusal: {e:?}"),
    }
}

fn refused(src: &str) -> String {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    match maca_backend_nix::emit_checked(&p.module) {
        Ok(s) => panic!("expected a refusal, got:\n{s}"),
        Err(e) => e.join("\n"),
    }
}

#[test]
fn addition_is_lowered_not_nulled() {
    let out = nix("services.ssh.port = 8000 + 22\n");
    assert!(out.contains("(8000 + 22)"), "no sum in:\n{out}");
    assert!(!out.contains("= null"), "still null:\n{out}");
}

#[test]
fn subtraction_and_multiplication_are_lowered() {
    let out = nix("services.ssh.port = (100 - 1) * 2\n");
    assert!(out.contains("((100 - 1) * 2)"), "{out}");
}

#[test]
fn division_is_spelled_out_rather_than_a_slash() {
    let out = nix("services.ssh.port = 100 / 4\n");
    assert!(out.contains("builtins.div 100 4"), "{out}");
}

#[test]
fn remainder_is_lowered_since_nix_has_no_modulo() {
    let out = nix("services.ssh.port = 100 % 7\n");
    assert!(
        out.contains("builtins.div 100 7"),
        "remainder is not built from div:\n{out}"
    );
    assert!(out.contains("100 - "), "{out}");
}

#[test]
fn comparison_and_logic_are_lowered() {
    let out = nix("services.ssh.enable = 1 < 2 && true\n");
    assert!(out.contains("(1 < 2)"), "{out}");
    assert!(out.contains("&&"), "{out}");
}

#[test]
fn a_ternary_becomes_if_then_else() {
    let out = nix("services.ssh.port = true ? 22 : 2222\n");
    assert!(
        out.contains("if true then 22 else 2222"),
        "no conditional in:\n{out}"
    );
}

#[test]
fn negation_is_lowered() {
    let out = nix("services.ssh.port = -1\n");
    assert!(out.contains("(-1)"), "{out}");
}

#[test]
fn string_concatenation_uses_nix_plus_not_list_append() {
    let out = nix("services.ssh.banner = \"a\" ++ \"b\"\n");
    assert!(out.contains(" + "), "{out}");
    assert!(!out.contains("++"), "emitted a list append:\n{out}");
}

#[test]
fn shifts_are_refused_because_nix_has_none() {
    let msg = refused("services.ssh.port = 1 << 4\n");
    assert!(msg.contains("<<"), "message does not name it: {msg}");
    assert!(msg.contains("Nix"), "message does not say why: {msg}");
}

#[test]
fn a_call_is_refused_rather_than_nulled() {
    let msg = refused("services.ssh.port = double(11)\n");
    assert!(msg.contains("function call"), "{msg}");
}

#[test]
fn a_refusal_names_the_construct_not_the_generated_nix() {
    for src in [
        "services.ssh.port = 1 << 4\n",
        "services.ssh.port = double(11)\n",
    ] {
        let msg = refused(src);
        assert!(!msg.contains("null"), "refusal talks about output: {msg}");
    }
}

#[test]
fn a_plain_config_still_emits() {
    let out = nix("services.ssh.enable = true\nusers.alice.packages = git, ripgrep\n");
    assert!(out.contains("true"), "{out}");
    assert!(out.contains("pkgs.git"), "{out}");
}
