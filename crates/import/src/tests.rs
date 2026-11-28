//! Format tests. Every fixture here is written for these tests; none is copied
//! from a vendor's files.

use std::io::Write;

use proptest::prelude::*;
use vocalis_core::dictionary::Entry;
use vocalis_core::snippets::Snippet;

use super::*;

fn snip(t: &str, c: &str) -> Snippet {
    Snippet {
        trigger: t.into(),
        content: c.into(),
    }
}

fn run(format: Format, data: &str) -> ImportReport {
    import(format, data.as_bytes(), &[], &[]).expect("import")
}

fn reasons(r: &ImportReport) -> Vec<(String, SkipReason)> {
    r.skipped.iter().map(|s| (s.item.clone(), s.reason.clone())).collect()
}

fn unsupported(r: &ImportReport, item: &str) -> bool {
    r.skipped
        .iter()
        .any(|s| s.item == item && matches!(s.reason, SkipReason::Unsupported { .. }))
}

// ---------- Espanso ----------

const ESPANSO: &str = r#"
global_vars:
  - name: today
    type: date
    params:
      format: "%Y-%m-%d"
matches:
  - trigger: ":sig"
    replace: "Kind regards,\nRobin"
  - triggers: [":hi", ":hey"]
    replace: "Hi {{who}}!"
    vars:
      - name: who
        type: echo
        params:
          echo: "team"
  - trigger: ":stamp"
    replace: "Filed {{today}} $|$"
  - trigger: ":paste"
    replace: "Link: {{clip}}"
    vars:
      - name: clip
        type: clipboard
  - trigger: ":ip"
    replace: "{{out}}"
    vars:
      - name: out
        type: shell
        params:
          cmd: "curl example.invalid"
  - trigger: ":ghost"
    replace: "{{nothing}}"
  - regex: ":n(?P<x>\\d+)"
    replace: "{{x}}"
  - trigger: ":form"
    form: "Name: [[name]]"
  - trigger: ":md"
    markdown: "**bold** move"
  - trigger: ":num"
    replace: 42
  - replace: "orphan"
"#;

#[test]
fn espanso_maps_triggers_and_vars() {
    let r = run(Format::Espanso, ESPANSO);
    assert_eq!(
        r.snippets,
        vec![
            snip(":sig", "Kind regards,\nRobin"),
            snip(":hi", "Hi team!"),
            snip(":hey", "Hi team!"),
            snip(":stamp", "Filed {datetime} "),
            snip(":paste", "Link: {clipboard}"),
            snip(":md", "**bold** move"),
            snip(":num", "42"),
        ]
    );
    assert!(unsupported(&r, ":ip"));
    assert!(unsupported(&r, ":ghost"));
    assert!(unsupported(&r, ":form"));
    assert!(r.skipped.iter().any(|s| s.item == "match 7"
        && s.reason
            == SkipReason::Unsupported {
                feature: "regex triggers".into()
            }));
    assert!(r.skipped.iter().any(|s| s.item == "match 11"
        && s.reason
            == SkipReason::Invalid {
                detail: "no trigger".into()
            }));
    let changes: Vec<&str> = r
        .converted
        .iter()
        .filter(|c| c.item == ":stamp")
        .map(|c| c.change.as_str())
        .collect();
    assert_eq!(
        changes,
        vec![
            "date variable replaced by Vocalis's date and time",
            "cursor position removed"
        ]
    );
    assert!(r
        .converted
        .iter()
        .any(|c| c.item == ":md" && c.change.contains("Markdown")));
    assert!(r.entries.is_empty());
}

#[test]
fn espanso_malformed() {
    assert!(matches!(
        import(Format::Espanso, b"matches: [unclosed", &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
    assert!(matches!(
        import(Format::Espanso, b"- just\n- a list\n", &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
    assert!(matches!(
        import(Format::Espanso, b"matches: 3\n", &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
    assert!(matches!(
        import(Format::Espanso, b"imports:\n  - other.yml\n", &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
    assert_eq!(
        import(Format::Espanso, b"\xff\xfe", &[], &[]),
        Err(ImportError::NotText)
    );
    // An empty list is a valid, empty file.
    assert_eq!(run(Format::Espanso, "matches: []\n").imported(), 0);
}

// ---------- TextExpander ----------

#[test]
fn textexpander_csv() {
    let csv = "abbreviation,content,label\n\
               ;addr,\"12 Quay Road\nHarbourtown\",Address\n\
               ;dt,\"Sent %B %e, %Y at %H:%M\",Date\n\
               ;cb,Copied: %clipboard%|,Clip\n\
               ;fill,\"Dear %filltext:name=who%,\",Fill\n\
               ;pct,50% off,\n\
               ,no abbreviation,\n\
               ;only\n\
               \n";
    let r = run(Format::TextExpander, csv);
    assert_eq!(
        r.snippets,
        vec![
            snip(";addr", "12 Quay Road\nHarbourtown"),
            snip(";dt", "Sent {datetime} at {datetime}"),
            snip(";cb", "Copied: {clipboard}"),
            snip(";pct", "50% off"),
        ]
    );
    assert!(unsupported(&r, ";fill"));
    assert!(r.skipped.iter().any(|s| s.item == ";only"));
    assert!(r.skipped.iter().any(|s| s.item.starts_with("line ")));
    assert!(r
        .converted
        .iter()
        .any(|c| c.item == ";cb" && c.change.contains("cursor")));
}

#[test]
fn textexpander_too_long_and_empty() {
    let long = "x".repeat(10_001);
    let csv = format!("big,{long},\nempty,\"  \",\n");
    let r = run(Format::TextExpander, &csv);
    assert!(r.snippets.is_empty());
    assert_eq!(
        reasons(&r),
        vec![
            ("big".into(), SkipReason::TooLong),
            (
                "empty".into(),
                SkipReason::Invalid {
                    detail: "the content is empty".into()
                }
            ),
        ]
    );
}

// ---------- Raycast ----------

#[test]
fn raycast_json() {
    let json = r#"[
      {"name": "Signature", "text": "Cheers,\nAlex", "keyword": "!sig"},
      {"name": "Meeting note", "text": "Notes for {date format=\"d MMM\"} {cursor}"},
      {"name": "Paste", "text": "See {clipboard}", "keyword": "!cb"},
      {"name": "Tomorrow", "text": "Due {date offset=+1d}", "keyword": "!tm"},
      {"name": "Loud", "text": "{clipboard | uppercase}", "keyword": "!up"},
      {"name": "Sel", "text": "> {selection}", "keyword": "!q"},
      {"name": "Code", "text": "const a = {x: 1};", "keyword": "!code"},
      {"name": "No text", "keyword": "!none"},
      42
    ]"#;
    let r = run(Format::Raycast, json);
    assert_eq!(
        r.snippets,
        vec![
            snip("!sig", "Cheers,\nAlex"),
            snip("Meeting note", "Notes for {datetime} "),
            snip("!cb", "See {clipboard}"),
            snip("!code", "const a = {x: 1};"),
        ]
    );
    for t in ["!tm", "!up", "!q"] {
        assert!(unsupported(&r, t), "{t}");
    }
    assert!(r.skipped.iter().any(|s| s.item == "!none" || s.item == "No text"));
    assert!(r.skipped.iter().any(|s| s.item == "item 9"));
    let notes: Vec<&str> = r
        .converted
        .iter()
        .filter(|c| c.item == "Meeting note")
        .map(|c| c.change.as_str())
        .collect();
    assert_eq!(notes.len(), 3, "{notes:?}");
}

#[test]
fn raycast_malformed() {
    assert!(matches!(
        import(Format::Raycast, b"{\"name\": 1}", &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
    assert!(matches!(
        import(Format::Raycast, b"[{", &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
}

// ---------- Alfred ----------

fn alfred_json(name: &str, keyword: &str, snippet: &str) -> String {
    serde_json::json!({
        "alfredsnippet": { "snippet": snippet, "uid": format!("uid-{name}"), "name": name, "keyword": keyword }
    })
    .to_string()
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, data) in files {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn alfred_collection() {
    let plist = "<?xml version=\"1.0\"?><plist><dict>\
                 <key>snippetkeywordprefix</key>\n<string>;;</string>\
                 <key>snippetkeywordsuffix</key><string></string></dict></plist>";
    let a = alfred_json("Address", "addr", "4 Elm Row\nNorthby");
    let b = alfred_json("Stamp", "now", "At {datetime:short} {cursor}");
    let c = alfred_json("Older clip", "c1", "{clipboard:1}");
    let d = alfred_json("Plain name", "", "Uses {clipboard} and {unknown}");
    let e = alfred_json("Shift", "tmr", "{date +1d}");
    let data = zip_of(&[
        ("info.plist", plist.as_bytes()),
        ("Address [uid-a].json", a.as_bytes()),
        ("Stamp [uid-b].json", b.as_bytes()),
        ("Older [uid-c].json", c.as_bytes()),
        ("Plain [uid-d].json", d.as_bytes()),
        ("Shift [uid-e].json", e.as_bytes()),
        ("broken.json", b"{nope"),
        ("__MACOSX/._Address.json", b"\x00\x01"),
        ("icon.png", b"\x89PNG"),
    ]);
    assert_eq!(detect("Work.alfredsnippets", &data), Some(Format::Alfred));
    let r = import(Format::Alfred, &data, &[], &[]).unwrap();
    assert_eq!(
        r.snippets,
        vec![
            snip(";;addr", "4 Elm Row\nNorthby"),
            snip("Plain name", "Uses {clipboard} and {unknown}"),
            snip(";;now", "At {datetime} "),
        ]
    );
    assert!(unsupported(&r, ";;c1"));
    assert!(unsupported(&r, ";;tmr"));
    assert!(r.skipped.iter().any(|s| s.item == "broken.json"));
}

#[test]
fn alfred_single_json_and_malformed() {
    let one = alfred_json("Hello", "hlo", "Hello there");
    assert_eq!(detect("x.json", one.as_bytes()), Some(Format::Alfred));
    let r = import(Format::Alfred, one.as_bytes(), &[], &[]).unwrap();
    assert_eq!(r.snippets, vec![snip("hlo", "Hello there")]);
    assert!(matches!(
        import(Format::Alfred, b"PK\x03\x04garbage", &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
    let empty = zip_of(&[("readme.txt", b"hi")]);
    assert!(matches!(
        import(Format::Alfred, &empty, &[], &[]),
        Err(ImportError::Malformed { .. })
    ));
}

#[test]
fn alfred_refuses_zip_bombs() {
    // Highly compressible content larger than the unpack budget.
    let big = vec![b' '; 21 * 1024 * 1024];
    let data = zip_of(&[("a.json", &big)]);
    assert!(data.len() < MAX_FILE_BYTES);
    assert_eq!(import(Format::Alfred, &data, &[], &[]), Err(ImportError::TooLarge));
}

// ---------- Wispr Flow ----------

#[test]
fn wispr_dictionary_csv_rules() {
    let csv = "Vocalis\n\
               \"Whisper Flow\",\"Wispr Flow\"\n\
               postgres,PostgreSQL,\n\
               word,\n\
               \u{200B}  ,kubectl\n\
               don\"t,do not\n\
               a,b,c\n\
               \n   \n\
               ,,\n\
               vocalis\n";
    let r = run(Format::WisprFlow, csv);
    assert_eq!(
        r.entries,
        vec![
            Entry::vocab("Vocalis"),
            Entry::correction("Whisper Flow", "Wispr Flow"),
            Entry::correction("postgres", "PostgreSQL"),
            Entry::vocab("word"),
            Entry::vocab("kubectl"),
            Entry::correction("don\"t", "do not"),
        ]
    );
    assert_eq!(
        reasons(&r),
        vec![
            (
                "line 7".into(),
                SkipReason::Invalid {
                    detail: "more than two columns".into()
                }
            ),
            ("vocalis".into(), SkipReason::Duplicate),
        ]
    );
}

#[test]
fn wispr_lengths_follow_our_limits() {
    let long = "w".repeat(51);
    let r = run(Format::WisprFlow, &format!("{long}\nok,{long}\n"));
    assert!(r.entries.is_empty());
    assert!(r.skipped.iter().all(|s| s.reason == SkipReason::TooLong));
    assert_eq!(r.skipped.len(), 2);
}

#[test]
fn wispr_snippets_json() {
    let json = r#"[
      {"name": " my address ", "text": "7 Mill Lane", "extra": true},
      {"name": "sig", "text": "  "},
      {"name": 5, "text": "x"},
      "not an object",
      {"name": "MY ADDRESS", "text": "dupe"}
    ]"#;
    assert_eq!(detect("snippets.json", json.as_bytes()), Some(Format::WisprFlow));
    let r = run(Format::WisprFlow, json);
    assert_eq!(r.snippets, vec![snip("my address", "7 Mill Lane")]);
    assert_eq!(r.skipped.len(), 4);
    assert_eq!(
        r.skipped[3],
        Skipped {
            item: "MY ADDRESS".into(),
            reason: SkipReason::Duplicate
        }
    );
}

#[test]
fn wispr_unterminated_quote_is_not_a_crash() {
    let r = import(Format::WisprFlow, b"ok\n\"open,never closed\nnext\n", &[], &[]);
    match r {
        Ok(r) => assert_eq!(r.entries.first(), Some(&Entry::vocab("ok"))),
        Err(e) => assert!(matches!(e, ImportError::Malformed { .. })),
    }
}

// ---------- Superwhisper (conservative CSV) ----------

#[test]
fn superwhisper_csv_with_header() {
    let csv = "word,original\nPostgreSQL,postgres\nKubernetes\n";
    let r = run(Format::Superwhisper, csv);
    assert_eq!(
        r.entries,
        vec![Entry::correction("postgres", "PostgreSQL"), Entry::vocab("Kubernetes")]
    );
    assert_eq!(r.converted[0].change, "header row skipped");
    let r = run(Format::Superwhisper, "original,replacement\nvoka lis,Vocalis\n");
    assert_eq!(r.entries, vec![Entry::correction("voka lis", "Vocalis")]);
}

// ---------- shared behaviour ----------

#[test]
fn dedups_against_existing_library() {
    let existing = [Entry::vocab("Vocalis"), Entry::correction("postgres", "PostgreSQL")];
    let existing_snips = [snip("My Sig", "x")];
    let r = import(
        Format::WisprFlow,
        b"vocalis\nPOSTGRES,postgresql\nnew word\n",
        &existing,
        &existing_snips,
    )
    .unwrap();
    assert_eq!(r.entries, vec![Entry::vocab("new word")]);
    assert_eq!(
        r.skipped.iter().filter(|s| s.reason == SkipReason::Duplicate).count(),
        2
    );
    let r = import(
        Format::Raycast,
        br#"[{"name":"a","text":"b","keyword":"my sig!"}]"#,
        &existing,
        &existing_snips,
    )
    .unwrap();
    assert!(r.snippets.is_empty());
    assert_eq!(r.skipped[0].reason, SkipReason::Duplicate);
}

#[test]
fn detects_formats() {
    assert_eq!(detect("base.yml", b"matches: []"), Some(Format::Espanso));
    assert_eq!(detect("x", b"matches:\n  - trigger: a\n"), Some(Format::Espanso));
    assert_eq!(
        detect("x.json", br#"[{"name":"a","text":"b","keyword":"k"}]"#),
        Some(Format::Raycast)
    );
    assert_eq!(
        detect("group.csv", b"sig,Best wishes,Signature\n"),
        Some(Format::TextExpander)
    );
    assert_eq!(detect("dict.csv", b"postgres,PostgreSQL\nVocalis\n"), None);
    assert_eq!(detect("dict.csv", b"hello,world,\n"), None);
    assert_eq!(detect("x.json", b"{\"a\":1}"), None);
    assert_eq!(
        import_auto(None, "dict.csv", b"a,b\n", &[], &[]),
        Err(ImportError::UnknownFormat)
    );
}

#[test]
fn size_and_item_limits() {
    let big = vec![b'a'; MAX_FILE_BYTES + 1];
    assert_eq!(import(Format::WisprFlow, &big, &[], &[]), Err(ImportError::TooLarge));
    let many: String = (0..=MAX_ITEMS).map(|i| format!("w{i}\n")).collect();
    assert_eq!(
        import(Format::WisprFlow, many.as_bytes(), &[], &[]),
        Err(ImportError::TooManyItems)
    );
}

#[test]
fn report_serializes_for_the_ui() {
    let r = run(Format::Raycast, r#"[{"name":"n","text":"{uuid}","keyword":"k"}]"#);
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["format"], "raycast");
    assert_eq!(v["skipped"][0]["reason"]["kind"], "unsupported");
    assert_eq!(v["skipped"][0]["reason"]["feature"], "the {uuid} placeholder");
}

#[test]
fn labels_are_short() {
    assert_eq!(label("one\ntwo"), "one…");
    assert_eq!(label(&"x".repeat(70)).chars().count(), 61);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// No input can make a parser panic, and every imported item passes core validation.
    #[test]
    fn never_panics_and_output_is_valid(data in proptest::collection::vec(any::<u8>(), 0..400), pick in 0usize..6) {
        let format = Format::ALL[pick];
        if let Ok(r) = import(format, &data, &[], &[]) {
            for (i, e) in r.entries.iter().enumerate() {
                prop_assert!(vocalis_core::dictionary::validate(e, &r.entries[..i], None).is_ok());
            }
            for s in &r.snippets {
                prop_assert!(vocalis_core::snippets::validate(s).is_ok());
            }
        }
    }

    #[test]
    fn text_inputs_never_panic(s in "\\PC{0,300}", pick in 0usize..6) {
        let _ = import(Format::ALL[pick], s.as_bytes(), &[], &[]);
        let _ = detect("f.csv", s.as_bytes());
    }
}
