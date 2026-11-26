# Spec 4.4: Custom tools (MCP client) and the Agents beta

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.4 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Model:** Commands can call **local MCP servers** that the user adds. These are small, single-purpose tools, deliberately local so a **fast model with few tools** stays at about 1 s latency. The reference argues against loading big cloud MCP servers with dozens of tools.

**Settings → MCP servers**
- **Import folder:** pick a folder containing `mcp.json` or `.mcp.json`. The server runs **from that folder** (no absolute paths needed).
- **New server** wizard, 5 steps:
  1. Location (default `~/Vocalis/servers`).
  2. Language: **Python with uv and FastMCP**, or **TypeScript with Bun and the MCP SDK**. The wizard installs and locks dependencies.
  3. Runtime check: find uv or bun. If missing, show the one-line install command and a **Check again** button. If several copies exist, let the user pick one.
  4. Name, with collision handling.
  5. Hand off: a copyable prompt for a coding agent, plus Reveal in Finder/Explorer. The folder is **watched**, so tools appear as the agent writes them.
- **Edit config:** opens `~/Vocalis/mcp.json`, in the standard `mcpServers` format with `command`, `args` and `env`. **Hot-reload on change.**
- **Server card:** icon, name, one-line description, up to 3 tool chips plus "+N more", and an enable switch. A failed server gets a warning badge.
- **Detail dialog:** status dot, every tool with its approval policy, the failure message, the effective config, and Refresh tools / Open folder / Delete.
- **Approval policy per tool:** **Auto** (runs immediately), **Ask** (approval card), **Deny** (the model is told the call was declined). Tools with read-only hints default to Auto; everything else defaults to Ask.
- **Approval card:** "*Server* wants to run *tool*" with Deny / Always allow / Allow once. **Enter = Allow once** and never pastes. After a **60 s timeout** nothing runs; the window says it stopped waiting and offers a retry.
- **Failure taxonomy.** Show a plain-language cause instead of a log:
  1. runtime not found
  2. missing package
  3. missing env var
  4. couldn't launch (folder moved)
  5. started but didn't speak MCP (usually stdout noise)
  6. one tool has an invalid schema (the rest still work)
  7. unknown error

  Include the exit code when there is one. Causes 1–3 are detected by pre-checks; for the others, show the server's stderr in the dialog.
- **Companion "skill" repo for coding agents** (our own, e.g. `Vocalis-skills`): instructions for building servers, a validator script (`validate_mcp.py`) that boot-tests a server, and a **hello-world weather example** (Open-Meteo, no API key). Docs include a copy-paste prompt for Claude Code/Codex/Cursor, and a "For coding agents" section with exact commands and the required final hand-off message (the import steps plus one suggested voice command).
- **Use-case proof:** an accounting tool (list invoices awaiting payment, plus record a payment, gated with Ask and an approval card showing the customer, amount, date and account), and a music tool (start a playlist).

**Agents (beta, Settings → Agents):** hand a spoken task to **Claude Code or Codex** running locally. The reference shows this only as a beta settings row, so spec it as: pick the agent, pick the working directory, the spoken brief becomes an agent session, and status shows in History. Mark UNKNOWN details in the ledger.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
