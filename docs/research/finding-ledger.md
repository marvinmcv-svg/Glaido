# Finding ledger (Phase R)

Method: §3.2 of the master prompt. Columns: `ID | Question | Conclusion | Evidence | Type | Confidence | Unknowns`.

**Evidence key.** `AUDIT-2026-11-26 §x.y` = the dated public-page audit in `docs/MASTER_PROMPT.md` (desktop v0.5.5). Rows sourced only from the audit are *observed (audit)*, medium confidence, until re-verified with an REA capture (ADR-0009). Our own design choices are **not** recorded here; they go in `docs/DECISIONS.md`.

| ID | Question | Conclusion | Evidence | Type | Confidence | Unknowns |
|---|---|---|---|---|---|---|
| L-001 | Q1 dictation lifecycle | Hotkey → start sound + bar → capture with noise reduction → stream to STT → on stop, formatting LLM → paste at focus → local History record | AUDIT §4.1 | observed (audit) | medium | Exact timing of sound vs. capture start |
| L-002 | Q1 states | Bar states: recording, processing, slow connection, nearing limit, error | AUDIT §4.1 | observed (audit) | medium | Visual specifics (not to be copied) |
| L-003 | Q1 limits | 10-minute cap, warning before cutoff, auto-stop still transcribes | AUDIT §4.1 | observed (audit) | medium | Warning lead time (we chose 9:30, §5.2) |
| L-004 | Q1 resilience | Crash, stall, mic loss, lock, network drop, timeout and quota each yield a failed record with Retry; audio kept | AUDIT §4.1 | observed (audit) | medium | Journal format (our design) |
| L-005 | Q1 paste | Clipboard + synthetic paste, restore unless user copied since; optional copy-to-clipboard; paste into starting app (mac/win); nowhere-to-paste notice (mac); Linux terminals use Ctrl+Shift+V | AUDIT §4.1 | observed (audit) | medium | Restore delay |
| L-006 | Q1 wake word | Wake word or "Hey <wake word>" in first two words (case-insensitive) → strip and run as Command; name-only opens command window | AUDIT §4.1 | observed (audit) | medium | Tokenization of punctuation after the wake word |
| L-007 | Q2 hotkeys | Defaults per OS as in Appendix A; hold, toggle, double-tap-hold, double-tap toggle; Esc cancels hands-free; optional Enter stops and pastes | AUDIT §4.1, App. A | observed (audit) | medium | Tap window (we chose 300 ms) ; conflict rules |
| L-008 | Q3 formatting | Three sections (All apps, Email, custom); styles Standard/Casual/Lowercase; Raw text; 500-char prompt; All-apps prompt inherited first; one app/site per rule | AUDIT §4.5.3 | observed (audit) | medium | Exact Casual punctuation rule |
| L-009 | Q3 snippets | Fire only when whole dictation equals trigger, ignoring case and edge punctuation; first wins; not applied to commands | AUDIT §4.5.2 | observed (audit) | medium | — |
| L-010 | Q3 dictionary | Vocab and correction entries; 50-char limit; duplicate rejection; tags `{clipboard}` `{datetime}` `{timezone}` make an entry correction-only; suggestions after 2 fixes of ≥4-char word, 30-day expiry | AUDIT §4.5.1 | observed (audit) | medium | Datetime format |
| L-011 | Q4 commands loop | Select text → hold commands key → stream Markdown → Enter pastes / Esc backgrounds; refine keeps context; per-turn context chips | AUDIT §4.2 | observed (audit) | medium | Max tool iterations |
| L-012 | Q4 tools | 10 built-in tools in three groups, each with a switch; image analysis asks first | AUDIT §4.3 | observed (audit) | medium | Search provider |
| L-013 | Q4 custom tools | Local MCP servers via mcp.json; Auto/Ask/Deny; approval card with 60 s timeout; 7-cause failure taxonomy | AUDIT §4.4 | observed (audit) | medium | Agents beta details |
| L-014 | Q5 history | Grouped by day; search ≥2 chars, ≤50 results; encrypted local DB; `.wav.enc`; audio kept for last 100 recordings | AUDIT §4.6 | observed (audit) | medium | Cipher parameters (our design) |
| L-015 | Q6 extensions | Six built-in extensions with manifests, permissions, Spotlight views and settings | AUDIT §4.7 | observed (audit) | medium | See spec 4-7 |
| L-016 | Q7 CLI/MCP | CLI over local IPC; MCP server with read-only mode and in-app approval for non-additive writes | AUDIT §4.9 | observed (audit) | medium | IPC transport |
| L-017 | Q8 plans | Free weekly word quota; Pro monthly/annual per seat; teams; referrals | AUDIT §4.12 | observed (audit) | medium | Exact referral reward rules |
| L-018 | Q9 site IA | Home, pricing, business, about, docs, use-cases, comparisons, extensions, legal, trust center | AUDIT §4.13 | observed (audit) | medium | Re-verify with R1 capture |
| L-019 | Q10 platforms | macOS 13+, Windows 11, Linux .deb/.rpm (Wayland and X11), iOS 18+, Android | AUDIT §4.0, §4.10–4.11 | observed (audit) | medium | — |

## Checklist status

| Q | Status |
|---|---|
| Q1–Q10 | **partial**: seeded from audit, awaiting REA re-verification (PLAN O5) |
