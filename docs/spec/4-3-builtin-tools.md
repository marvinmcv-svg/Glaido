# Spec 4.3: Built-in tools for Commands (10, each on by default with its own switch)

> **Status:** stub, seeded from `docs/MASTER_PROMPT.md` §4.3 (reference audit of 2026-11-26).
> **Clean-room note:** this is a *functional* spec. Implementers build from this file only. Any quoted phrase is descriptive; all shipped copy must be original.
> **Ledger:** behaviors here map to rows in `docs/research/finding-ledger.md`. Anything not stated is `UNKNOWN` until a ledger row or an ADR settles it.
> **Product variables:** placeholders from the master prompt are already substituted with the ADR-0001 defaults (product and wake word "Vocalis", CLI `vocalis`, domain vocalis.app).

Settings → **Built-in tools**, grouped into three categories. Each card opens a dialog with a description and example phrases. When a tool is unavailable (offline, unsupported OS) its card dims, loses its switch, and explains why. If a whole category is unavailable it shows "Not available yet".

| Group | Tool | Runs | Notes |
|---|---|---|---|
| Web | **Web search** | Cloud (search API, e.g. Exa/Brave/Tavily) | Reads the top results and answers in prose, not a list of links |
| Web | **Read a page** | Cloud | Defaults to the **page open in the frontmost browser**, or a selected link |
| Web | **Summarize a YouTube video** | Cloud | Uses the transcript; requires captions |
| Web | **Deep research** | Cloud, async (minutes) | Runs in the background. The write-up lands in History **with links to every source**, and a notification fires |
| System | **Math and dates** | Local, exact | Percentages, arithmetic, time zones, date offsets ("17.5% of 849", "10 days from now", "9am Pacific in my time") |
| System | **Files and apps** | Local | Find, list, read and open files, folders and apps by **description** ("the contract I saved last month", "open that screenshot in Preview"). Runs without asking; can be switched off |
| System | **Search clipboard history** | Local | The last **24 h, up to 500 items**. Cleared together with history |
| System | **Analyze image** | Cloud, **asks first** | Shows the copied picture with **Deny / Always allow / Allow once**. Once approved, the image stays attached for follow-ups. An approval policy setting exists. Never analyzes other copied images |
| App | **Search your history** | Local | Fuzzy, describes-what-you-said search. Pastes the closest match |
| App | **Search documentation** | Cloud (reads our docs) | Answers "how do I…" questions about our app |

Extensions (§4.7) register additional tools ("pick a color", "keep awake for an hour", "kill Slack").

## Open questions / UNKNOWN

- _(add as found; link a ledger row or ADR)_
