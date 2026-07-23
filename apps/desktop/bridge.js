// Tauri glue: wires the Maca UI (app.js) to the Maca native backend command.
//
// In a packaged Tauri build, `invoke` is `window.__TAURI__.invoke`, and the
// Rust command handler runs the compiled `backend` binary. For headless tests,
// a `globalThis.invoke` stub spawns that same binary directly — same contract.
(function () {
    const invoke =
        typeof globalThis.invoke === "function"
            ? globalThis.invoke
            : (cmd, arg) => window.__TAURI__.invoke(cmd, { arg });

    const go = document.getElementById("go");
    const name = document.getElementById("name");
    const result = document.getElementById("result");
    if (!go) return;
    go.addEventListener("click", async () => {
        const out = await Promise.resolve(invoke("greet", name.value));
        result.textContent = out;
    });
})();
