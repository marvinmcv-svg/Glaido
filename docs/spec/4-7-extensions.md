# Spec 4.7: Extensions (desktop; free on every plan)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.7 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

**Framework**
- Each extension has: a manifest of **declared permissions** (it can use nothing else), OS support flags, commands (each with optional hotkey, Spotlight keyword, and voice phrases), and settings.
- **Setup flow when turning one on:** 1. Permissions (what it can use). 2. System access (only if the OS requires a grant; Check again). 3. Hotkey (**unset by default**; conflict detection names who owns the keys). 4. Try it (sample voice phrases).
- **Three ways to use any extension:** its hotkey, Spotlight, or voice in Commands.
- **Deep links** (`vocalis://extensions/<id>/setup`) let website pages offer an "Open in app" button that jumps straight to setup.
- Off by default, except **Clipboard History, which is on by default**. The Extensions page has category filters: Productivity, Design, Your Computer, Developer, Built in.
- CLI: `vocalis extensions list|enable|disable`, `vocalis ext <id> <command> [args]`. An extension that is off or unsupported refuses.
- Permission vocabulary: Read a color on screen · Write to the clipboard · Show notifications · Use a global hotkey · Add tools to Command mode · Open and share items · Read what you type · Prevent sleep · See and end processes.

#### 4.7.1 Spotlight (launcher)
- A centered window over all apps. It has **no shortcut until one is set**. Press it again, Esc, or click away to close.
- **Empty state:** most-opened apps, plus views from enabled extensions.
- **Apps:** Enter opens; ⌘/Ctrl+Return reveals in its folder.
- **Files and folders:** name match within the home folder, plus iCloud Drive on macOS. Exclude source code and files inside project folders. Content search is left to the Commands "Files and apps" tool.
- **Calculator** (type the whole phrase; Enter copies the result and the window stays open; no currency because it needs live rates):
  - Arithmetic with `+ - * / % ( )`.
  - "52% of 900", "20% off 80", "square root of 625", "2^10" / "2 power 10".
  - Units: length, weight, temperature ("10ft in m", "70kg in lb", "20c in f").
  - CSS units: "2rem in px" at 16 px per rem, "2 inches in px at 72 ppi".
  - Spans: "145 mins to timespan".
  - Dates: "today", "tomorrow", "monday in 3 weeks", "august 5 + 5", "days until 31 mar".
  - Times: "time", "time in tokyo", "time in 4 hours", "12pm here in london".
  - ISO timestamps converted to local time.
- **Web:** `g`, `yt` and `gh` keywords (Google, YouTube, GitHub). The last row is always a web search. Typing a keyword alone offers it, and Enter fills it in.
- **History:** Up on the first row walks the **last 30 searches** (stored locally); Down returns to the current input.
- **Bookmarks:** keyword `bm` plus a space searches browser bookmarks.
- **Extension views by keyword:** `clipboard`, `kill`, `colors`, `snippets`, `awake`, `pick`. Inside a view, the field filters, Enter or double-click runs, Esc goes back, and a second Esc closes.
- **Dictate into Spotlight:** dictation goes into the search field. A command started while Spotlight is open sees what Spotlight shows (the query and rows, or the highlighted row), so "rewrite this" works on a highlighted clipboard item.

#### 4.7.2 Color Picker
- **Pick:**
  - macOS uses the system color sampler (no Screen Recording permission needed).
  - Windows samples on the next click, swallows that click, and Esc or right-click cancels.
  - Linux uses the XDG screenshot portal color picker, with an X11 fallback of next-click sampling.
- Converts to **sRGB** before formatting, so wide-gamut displays don't shift the hex.
- **Formats:** `#RRGGBB`, `RRGGBB`, `rgb()`, `hsl()`, `oklch()`. Uppercase hex toggle. A toast with the swatch (toggle).
- **Recent colors:** the last **12**, as a grid in Spotlight, with Clear recents.
- **Convert:** between formats and from **CSS color names** ("royal blue"). Name the nearest CSS color on request. Only the hex value is sent to the model.
- Voice: "pick a color", "pick a color as RGB", "what color did I pick last?", "convert #2563EB to HSL".

#### 4.7.3 Clipboard History
- Captures text, links, images, files and colors, stored locally and never synced (**★ BETTER:** optional E2E-encrypted sync).
- **Skips concealed, transient and auto-generated copies** (password managers) unless **Keep transient copies** is on.
- The Spotlight view shows the **newest 40 items, grouped by day**, a type filter (text, images, files, links, colors), and a preview pane where images can be dragged out and links open.
- **Enter or double-click pastes** into the previous app; ⌘/Ctrl+Enter copies instead (swappable). The Actions menu (⌘K) has Open, plus Open With and Share on macOS.
- **Settings:** Keep history (Day [default] / Week / Month / Year / Forever), Keep transient copies (off), Enter action (Paste / Copy), Erase history. Turning the extension off does *not* erase.
- Voice: "show my clipboard history", "what did I just copy?" (answers without opening a window), "find the link I copied", "show the images I copied", and "rewrite this" on a highlighted row. Image questions send only a title unless Analyze image is approved.
- Outside macOS, an image may paste as a file path when the platform can't put image data on the clipboard.

#### 4.7.4 Text Snippet Expansion
- Reuses the Snippets list. **Off by default** because it reads keystrokes.
- A **marker key** must be set; nothing expands without it. **Marker position:** *After the trigger* (default) or *Before the trigger*.
- The trigger must be a **whole word at the end** of the typed buffer. Matching ignores case, the **longest match wins**, and **one-letter triggers never fire from the keyboard** (they can still be spoken). With no match, the marker key behaves normally.
- Expansion pastes the snippet and restores the clipboard.
- The buffer holds only characters typed since the last click, app switch, Esc or arrow key; nothing is stored or sent. It **pauses** while our app is frontmost, while Spotlight is open, and while recording.
- Linux: if only the shortcut portal is available, say so on the extension page. Letter keys follow US positions.
- Voice and Spotlight: "show my snippets" lists and filters them; "insert my email signature" pastes it, and if several match, show them and paste nothing.

#### 4.7.5 Processes
- The Spotlight view (`kill`) lists named processes with CPU and memory, a name filter, and a sort toggle (CPU by default, or memory).
- Enter asks "End *X*?" at the bottom; Enter again confirms and Esc goes back. Actions: **Force quit**, **Restart** (end and relaunch), **Copy path**, **Refresh**, each with its own confirmation.
- Voice: "what's using my CPU?" (opens the list; the data is *not* sent to the model), "kill WhatsApp", "force quit Notes", "restart WhatsApp". **Always** confirm with Allow once / Deny; there is no "always allow". With duplicate names, open the list to pick.
- **Safety:** never touch our own process or OS-critical processes (kernel, login and window servers, init). **Re-verify the PID or identity immediately before acting.** Names must match exactly. Processes owned by other users show a clear error.
- CLI and MCP calls skip the in-app confirmation; the calling agent's approval is the check.

#### 4.7.6 Keep Awake
- Prevents idle sleep and keeps the display on. Manual Sleep and closing the lid still work.
  - macOS uses IOPMAssertion.
  - Windows uses SetThreadExecutionState.
  - Linux uses logind or the inhibit portal (if neither exists, say so).
- **Sessions:**
  - A duration (30 min, 2 h, forever).
  - Until a time ("6pm", "Friday 6pm"; a time already past today means the next one).
  - A **weekly window** ("weekdays 9–6", "Mon–Fri except Wed 7am–7pm").

  A new request replaces the current one. Toasts announce start, change and end. **Allow sleep** ends a session.
- Spotlight presets: 1 hour / 1 day / 1 week / Forever.
- Voice: "keep my computer awake for 2 hours", "…until 6pm", "…on weekdays from 9 to 6", "let the computer sleep", "how long is keep awake running?"
- Only weekly windows persist (and resume after restart). Durations aren't stored. Turning the extension off deletes saved windows. We report and release only our own assertion.

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
