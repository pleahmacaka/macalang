//! Zed extension for Maca: registers the `maca-lsp` language server so Zed gets
//! live diagnostics, hover, and completion on top of the tree-sitter
//! highlighting in `languages/maca/`.
//!
//! The server binary (`maca-lsp`, built from `crates/lsp`) must be on PATH;
//! the install script places it next to `maca`.

use zed_extension_api::{self as zed, LanguageServerId, Result};

struct MacaExtension;

impl zed::Extension for MacaExtension {
    fn new() -> Self {
        MacaExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let path = worktree
            .which("maca-lsp")
            .ok_or_else(|| "`maca-lsp` not found on PATH; install it with the Maca installer".to_string())?;
        Ok(zed::Command { command: path, args: vec![], env: worktree.shell_env() })
    }
}

zed::register_extension!(MacaExtension);
