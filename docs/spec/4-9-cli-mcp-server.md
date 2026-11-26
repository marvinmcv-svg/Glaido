# Spec 4.9: CLI and MCP server (Developer)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.9 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

- **Settings → Command line:** a switch that installs `vocalis` on the PATH (the CLI ships inside the app; nothing else to install). The status reads **Ready**, or **Not on your PATH** with the exact line to add to the shell profile. The app must be running for changes to apply; tool listing works without it.
- **Commands:**
  - `status`
  - `dictionary list|add <word> [--misspelling X]|remove`
  - `snippets list|set <trigger> <content>|remove`
  - `formatting list|set <name> --style <standard|casual|lowercase> --app <App> [--site <domain>] [--prompt …] [--raw]|remove`
  - `settings [get|set <key> <value>]` (with no args, lists every key and value)
  - `extensions list|enable|disable`
  - `ext <id> <command> [--flags]`
  - `servers [guide|list|import|enable|disable]`
  - `apply <file.json> [--yes]` (preview the diff, then confirm)
  - `mcp [--read-only]`
  - `completions <shell>`
  - `--help`
- **Safety semantics:** adds are additive only. `snippets set` replaces silently. Removals, formatting changes and setting changes **ask first** (y/N), or need `--yes`.
- **MCP server (`vocalis mcp`)** lets agents read and change our app: dictionary, snippets, formatting, settings, extensions, running extension commands, and starting custom tools.
  - Writes beyond plain adds produce a **pending change the user approves in the app**.
  - `--read-only` refuses every write.
  - The Settings page shows **one-click connect** for Claude Code (`claude mcp add --scope user vocalis -- vocalis mcp`), Codex (`codex mcp add vocalis -- vocalis mcp`) and Cursor (JSON snippet), with a **Run it for me** button.
  - An optional snippet lets an agent add dictionary words without asking.
- Docs make the two directions explicit: **MCP = an agent changes our app**; **Custom tools = our app calls your server**.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
