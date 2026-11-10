//! Which AI coding agents run in a session, told by their processes: Claude
//! Code is also known from its own session records (with an address to
//! message it by); the others are known only to be running.

/// The agents recognised, as the kinds the pages show icons for.
pub(crate) const KINDS: [&str; 7] = ["claude", "codex", "gemini", "aider", "opencode", "cursor", "copilot"];

fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The agent a command line starts, if any: by the program's own name, or
/// (run through node, python, bun…) by its script's name or package folder.
pub(crate) fn kind_of(argv: &[&str]) -> Option<&'static str> {
    let first = base(argv.first()?);
    let by_name = |name: &str| -> Option<&'static str> {
        let n = name.trim_end_matches(".exe").trim_end_matches(".js").trim_end_matches(".mjs").trim_end_matches(".py");
        match n {
            "claude" => Some("claude"),
            "codex" => Some("codex"),
            "gemini" => Some("gemini"),
            "aider" => Some("aider"),
            "opencode" => Some("opencode"),
            "cursor-agent" => Some("cursor"),
            "copilot" => Some("copilot"),
            _ => None,
        }
    };
    if let Some(kind) = by_name(first) {
        // Helpers Claude Code runs for itself are not a session someone works in.
        let helper = kind == "claude" && argv.get(1).is_some_and(|a| ["bg-pty-host", "daemon", "attach", "mcp"].contains(a));
        return (!helper).then_some(kind);
    }
    if !matches!(first, "node" | "bun" | "deno" | "python" | "python3" | "uv" | "npx") {
        return None;
    }
    let script = argv.iter().skip(1).find(|a| !a.starts_with('-'))?;
    by_name(base(script)).or_else(|| {
        [("/claude-code/", "claude"), ("/@openai/codex/", "codex"), ("/gemini-cli/", "gemini"), ("/aider/", "aider"), ("/opencode", "opencode")]
            .iter()
            .find(|(part, _)| script.contains(part))
            .map(|(_, kind)| *kind)
    })
}

/// Kinds in a fixed order, each once.
pub(crate) fn tidy(mut kinds: Vec<String>) -> Vec<String> {
    kinds.retain(|k| KINDS.contains(&k.as_str()));
    kinds.sort_by_key(|k| KINDS.iter().position(|x| x == k));
    kinds.dedup();
    kinds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_are_told_by_their_command_lines() {
        assert_eq!(kind_of(&["claude", "--dangerously-skip-permissions", "-c"]), Some("claude"));
        assert_eq!(kind_of(&["/workspace/.npm-global/lib/node_modules/@anthropic-ai/claude-code/bin/claude.exe", "--session-id", "x"]), Some("claude"));
        assert_eq!(kind_of(&["node", "/usr/lib/node_modules/@openai/codex/bin/codex.js", "exec"]), Some("codex"));
        assert_eq!(kind_of(&["/usr/local/bin/codex"]), Some("codex"));
        assert_eq!(kind_of(&["node", "--no-warnings", "/opt/x/node_modules/@google/gemini-cli/dist/index.js"]), Some("gemini"));
        assert_eq!(kind_of(&["python3", "/home/a/.local/bin/aider", "--model", "x"]), Some("aider"));
        // Claude Code's own helpers, and ordinary programs, are no agent session.
        assert_eq!(kind_of(&["claude", "bg-pty-host", "--bg-pty-host"]), None);
        assert_eq!(kind_of(&["claude", "attach", "abc"]), None);
        assert_eq!(kind_of(&["bash", "-c", "claude"]), None);
        assert_eq!(kind_of(&["node", "server.js"]), None);
        assert_eq!(kind_of(&[]), None);
        assert_eq!(tidy(vec!["codex".into(), "claude".into(), "codex".into(), "nope".into()]), vec!["claude", "codex"]);
    }
}
