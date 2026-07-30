#[test]
fn explore() {
    let ui = "count = 0\n\nbump() {\n    count = count + 1\n    update()\n}\n\nmain() -> Element =>\n    div(class=\"p-6\", h1(class=\"text-xl\", \"Clicks: {count}\"), button(on:click=bump, \"click\"))\n";
    let j = maca_wasm::compile_json(ui, 0);
    let a = j.find("\"parseErrors\"").unwrap();
    let b = j.find("\"outputs\"").unwrap();
    println!("--- ui ---\n{}\n", &j[a..b]);
}
