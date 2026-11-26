# Spec 4.2: Commands (voice agent)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.2 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Purpose.** Speak an instruction instead of text: rewrite, translate, summarize, answer, or run a tool. **Nothing is pasted until the user confirms.**

**Flow**
1. Optionally select text in any app. It becomes "this", and is shown in the request bubble with its source app.
2. Hold the **commands hotkey** (default: Right ⌥ on macOS, Left Ctrl + Left Alt on Windows, Ctrl+Alt-based on Linux) and speak. Release.
3. The **command window** opens and streams the answer as Markdown.
4. **Enter** pastes at the cursor and closes the window. **Esc** closes the window *without cancelling*: the run continues in the background, writes to the same History record, and sends a **notification on completion**. Clicking the notification opens the record.
5. **Refine:** hold the commands hotkey again and speak ("make it shorter", "drop the last sentence"). The conversation context is kept across turns.

**Command window details**
- It hangs from the top edge or sits at the bottom. The style and position are configurable: island on the top edge, or opened from the dictation bar.
- It opens small, grows with the answer, and is draggable by its header.
- It stays on top without stealing focus.
- Smooth message animations and auto-scroll.
- **Click any response to copy it.** ⌘C copies the highlighted part, or the whole answer if nothing is highlighted, and a copy button sits under the answer (always copies everything, never moves focus; it's the way to copy on Windows).
- The footer shows the hotkey and **Refine**, and **Enter: Paste**.
- **Each turn shows the context it used:** selected text, an attached image, the source app.
- Tool calls render inline, with a **Stop** button on long-running calls. An **approval card** appears for gated tools (§4.4).

**Context sent per request (privacy-scoped):**
- the spoken request
- the selected text
- earlier turns of this conversation
- the frontmost app's name
- the **field kind** (chat, email, code, terminal, generic)
- the user's local time

None of it is stored server-side.

**Example intents to support well:** translate, make formal, fix grammar, summarize, convert to bullets, reply to this email, explain this code, a better word for X, a percentage or date math question, "what's in the screenshot I copied?", "summarize this video", web questions.

**Windows note:** Commands can't read a selection inside terminals, because synthetic Ctrl+C would interrupt the running program. Detect terminals and skip selection capture there.

**Commands in games and full-screen apps:** the window overlays the game, and the game keeps keyboard focus so the user can keep moving. Use case: "how do I craft a pickaxe in Java Edition?"

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
