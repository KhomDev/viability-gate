//! `vg init` — scaffold the input files a new target needs.
//!
//! The tool's largest adoption barrier is not installation, it is the inputs: a
//! researcher who installs the binary and runs `vg check` gets a verdict, but a
//! researcher who wants the pre-hunt gates has to hand-author three files whose
//! shape they have never seen. This generates all three, commented, so the tool
//! teaches its own input format.
//!
//! Nothing generated here contains target data. The templates are structure only;
//! the researcher fills in what they read off the program page.

use std::path::Path;
use std::process::ExitCode;

pub const LEDGER_TEMPLATE: &str = r#"# Program ledger — YOUR transcription of one program's terms.
#
# PRIVATE. This file will contain a real program name, and your judgement about
# its terms is not something to publish. Add it to .gitignore.
#
# Fill in only what you actually read off the program page. An entry you guessed
# is worse than a missing entry, because you will act on it. `vg` warns on an
# uncited floor for exactly this reason.
#
# Docs: https://github.com/KhomDev/bounty-economics

- id: REPLACE-ME                 # your slug for the program
  tier_shape: unknown            # critical_only | high_and_above | medium_and_above
                                 # | low_and_above | unknown
                                 # There is no `any`. A program that documents a
                                 # NARROW floor (critical_only / high_and_above)
                                 # must say so here; that shape pays nothing below
                                 # its bar.
                                 # NOTE: a program with NO STATED floor is NOT
                                 # automatically low_and_above. Leave tier_shape as
                                 # unknown and set floor_status: unstated. An
                                 # unstated floor is not evidence that Low pays; it
                                 # is evidence the page has not been read.
  floor_status: unstated         # documented | unstated | conflicting
  floor_severity: null           # lowest severity with a NON-ZERO reward, or null.
                                 # null with floor_status: unstated is honest;
                                 # null with floor_status: documented is a guess.
  source_url: ""                 # the live rewards page you read
  retrieved: ""                  # YYYY-MM-DD — terms change without notice

  # Optional but valuable: disagreements inside the program's own materials.
  # A scope table can label an asset Critical while the reward table pays it at
  # High. The REWARD table governs; record the conflict rather than resolving it
  # silently, because next time you will have forgotten which one you trusted.
  conflicts: []

  # Every out-of-scope clause, VERBATIM, plus a regex to test against your own
  # finding description. A match is a prompt to check, not an automatic reject —
  # over-trusting a regex manufactures false negatives.
  #
  # Clause classes: privileged_role | human_error | admin_input |
  # dos_no_extraction | front_run_only | best_practice | design_issue |
  # off_chain | third_party | user_input | known_issue | testing_limits |
  # non_standard_token | scope_boundary
  exclusions: []
    # - class: privileged_role
    #   clause: "<paste the clause verbatim>"
    #   pattern: "(owner|admin|capability|role)\\b"

  # Read the findings, not the table of contents. A prior audit naming a file is
  # not the same as covering your function.
  prior_audits: []
    # - firm: "<firm>"
    #   url: "<report url>"
    #   covers: ["sources/Vault.sol"]
    #   known_issues: ["<verbatim from the program's known-issues section>"]

  # The richest surface, and the one you are least likely to be duplicating:
  # what changed after the last audited commit.
  post_audit_delta: ""
"#;

pub const KNOWN_ISSUES_TEMPLATE: &str = r#"# Known-issues corpus — the duplicate catcher.
#
# PRIVATE. Contains real program specifics.
#
# Duplicates are the largest single rejection category and the one no pattern
# over your finding's prose can find, because the signal lives in an EXTERNAL
# corpus: the program's known-issues section and its published audit findings.
# This file is that corpus.
#
# A match is a HARD fail, because a known issue is Not Applicable rather than
# Informational — a materially worse outcome than an unpaid finding.
#
# SPECIFICITY, which is easy to get wrong in both directions:
#   TOO BROAD   "reward"          -> fires on half of all findings
#   TOO NARROW  "vault.move:552"  -> fires on nothing you will write
# The right level names the MECHANISM in words another author would also use.

known_issues: []
  # - id: KI-1
  #   summary: "<one line: what the accepted behaviour is>"
  #   source: "<url or 'program known-issues section, YYYY-MM-DD'>"
  #   patterns:
  #     - "<regex matching the mechanism>"
  #     - "<a second phrasing, if the first could miss>"
"#;

pub const SCOPE_TEMPLATE: &str = r#"# In-scope files — one path per line, exactly as the program lists them.
#
# PRIVATE. Delete this comment block once filled; blank lines and # comments are
# ignored by `vg`.
#
# Gate 3 traces the ROOT CAUSE of a finding into this list. Not the impact site —
# the root cause. They are frequently in different files, and a real bug whose
# root cause lives in a deploy script or a test utility is out of scope
# regardless of its impact.
#
# sources/Vault.sol
# sources/Oracle.sol
"#;

pub const README_TEMPLATE: &str = r#"# Target: REPLACE-ME

Working notes for one program. Keep this private.

## Gate status

Run `vg gates --ledger ledger.yaml --scope scope.txt --known-issues known-issues.yaml`
and paste the result here, so the next session does not re-derive it.

```
(paste `vg gates` output)
```

## Decision

- [ ] Floor: `<tier_shape>` — can my thesis plausibly reach it?
- [ ] Exclusions transcribed and tested against my intended bug class
- [ ] Scope tree generated; root causes traceable into it
- [ ] Prior audits read (findings, not titles); known issues extracted
- [ ] Test suite grepped for intent markers

## Notes

"#;

/// Your own submission history. This is the input for gate G7.
///
/// It is LOCAL and gitignored on purpose. It contains your own bug families and
/// the outcomes you got, which is exactly the information a regex over a draft
/// finding cannot recover.
pub const HISTORY_TEMPLATE: &str = r#"# Your submission history — LOCAL. Never publish this file.
#
# Gate G7 compares a draft finding against this list and reports a probable
# repeat. It exists because a text pattern cannot see your own earlier
# submissions: in the source log behind this tool, 12 of 97 rejections repeat a
# family that was already present.
#
# Add one entry per submission. The family label is yours — write it the way you
# would describe the mechanism to yourself.
#
#   vg log add --family "stale oracle bound" --outcome duplicate \
#              --note "cross-vault replay, closed as known"

submissions: []

# Example entry:
# submissions:
#   - family: "stale cached exchange rate"
#     outcome: rejected          # paid | rejected | duplicate | oos | informational | pending
#     note: "closed as a known design tradeoff"
#     date: 2026-10-01
"#;

const FILES: &[(&str, &str)] = &[
    ("ledger.yaml", LEDGER_TEMPLATE),
    ("known-issues.yaml", KNOWN_ISSUES_TEMPLATE),
    ("scope.txt", SCOPE_TEMPLATE),
    ("history.yaml", HISTORY_TEMPLATE),
    ("NOTES.md", README_TEMPLATE),
];

pub fn run(dir: &Path, force: bool) -> ExitCode {
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("vg: cannot create {}: {e}", dir.display());
        return ExitCode::from(2);
    }

    let mut written = Vec::new();
    let mut skipped = Vec::new();

    for (name, body) in FILES {
        let path = dir.join(name);
        if path.exists() && !force {
            skipped.push(name.to_string());
            continue;
        }
        if let Err(e) = std::fs::write(&path, body) {
            eprintln!("vg: cannot write {}: {e}", path.display());
            return ExitCode::from(2);
        }
        written.push(name.to_string());
    }

    println!("scaffolded target inputs in {}\n", dir.display());
    for name in &written {
        println!("  wrote   {name}");
    }
    for name in &skipped {
        println!("  kept    {name}  (exists; --force to overwrite)");
    }

    println!(
        "\nnext:\n  \
         1. Open the program's REWARDS page (not the impact table) and fill in\n     \
            tier_shape + floor_severity + source_url + retrieved in ledger.yaml.\n  \
         2. Paste every exclusion clause verbatim and write a pattern for each.\n  \
         3. Fill scope.txt with the in-scope file list.\n  \
         4. Extract known issues and prior-audit findings into known-issues.yaml.\n  \
         5. Run:\n       \
            vg gates --ledger {d}/ledger.yaml --scope {d}/scope.txt \\\n         \
                     --known-issues {d}/known-issues.yaml\n\n  \
         The scaffold is INTENDED to fail validation until you fill it in — an\n  \
         empty source_url is an uncited floor, and an uncited floor is a guess.\n\n  \
         Unknown is not pass: a gate whose input you did not supply reports '?',\n  \
         never a tick. A gate you could not run is a gate you have not run.",
        d = dir.display()
    );
    ExitCode::SUCCESS
}

/// Append one submission to the local history log.
///
/// The file is plain YAML and is created on first use. It is never written
/// anywhere else, and the tool has no network code at all.
pub fn log_add(
    path: &Path,
    family: &str,
    outcome: &str,
    note: Option<&str>,
    date: Option<&str>,
) -> Result<(), String> {
    let mut entries: Vec<crate::model::LogEntry> = if path.exists() {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        parse_log(&text)?
    } else {
        Vec::new()
    };

    let today = date.map(|d| d.to_string()).unwrap_or_else(today_iso);
    entries.push(crate::model::LogEntry {
        family: family.to_string(),
        outcome: outcome.to_string(),
        note: note.map(|n| n.to_string()),
        date: Some(today),
    });

    let mut body = String::from(
        "# Your submission history - LOCAL. Never publish this file.\n\
         # Written by 'vg log add'. Gate G7 reads it.\n\nsubmissions:\n",
    );
    for e in &entries {
        body.push_str(&format!("  - family: {}\n", yaml_quote(&e.family)));
        body.push_str(&format!("    outcome: {}\n", yaml_quote(&e.outcome)));
        if let Some(n) = &e.note {
            body.push_str(&format!("    note: {}\n", yaml_quote(n)));
        }
        if let Some(d) = &e.date {
            body.push_str(&format!("    date: {}\n", yaml_quote(d)));
        }
    }
    std::fs::write(path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

fn parse_log(text: &str) -> Result<Vec<crate::model::LogEntry>, String> {
    if let Ok(list) = serde_yaml::from_str::<Vec<crate::model::LogEntry>>(text) {
        return Ok(list);
    }
    #[derive(serde::Deserialize)]
    struct Wrapper {
        #[serde(default)]
        submissions: Vec<crate::model::LogEntry>,
    }
    let w: Wrapper = serde_yaml::from_str(text).map_err(|e| format!("{e}"))?;
    Ok(w.submissions)
}

/// Quote a YAML scalar if it contains anything that would need it.
fn yaml_quote(s: &str) -> String {
    let needs = s.is_empty()
        || s.chars().any(|c| {
            !(c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' || c == '/')
        });
    if needs {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// Today as YYYY-MM-DD, from the Unix epoch, without a date library.
fn today_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    // Civil-from-days, the inverse of the algorithm in engine::days_since.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_is_non_empty() {
        for (name, body) in FILES {
            assert!(!body.trim().is_empty(), "{name} template is empty");
        }
    }

    #[test]
    fn templates_carry_guidance_not_just_structure() {
        // A template with no comments teaches nothing. Each one should explain
        // at least the decision it is asking the researcher to make.
        for (name, body) in FILES {
            assert!(
                body.lines()
                    .filter(|l| l.trim_start().starts_with('#'))
                    .count()
                    >= 3,
                "{name} has too few comments to be self-teaching"
            );
        }
    }

    #[test]
    fn templates_name_no_target() {
        // The scaffold is structure only. If a real program, chain or module name
        // ever appears here it ships to every user.
        //
        // The author's OWN repository is allowed: pointing a researcher at the
        // documentation is not a disclosure. What must never appear is a target
        // repository, a platform name, or a chain.
        let banned = [
            "hackenproof",
            "immunefi",
            "cantina",
            "sherlock",
            "code4rena",
            "gitlab.com/",
        ];
        let own_repo = "github.com/khomdev/";

        for (name, body) in FILES {
            let low = body.to_lowercase();
            for token in banned {
                assert!(
                    !low.contains(token),
                    "{name} contains a banned token: {token}"
                );
            }
            // Any github.com URL must point at the author's own repo.
            for (i, _) in low.match_indices("github.com/") {
                let tail = &low[i..];
                assert!(
                    tail.starts_with(own_repo),
                    "{name} links to a third-party repository: {}",
                    &tail[..tail.len().min(48)]
                );
            }
        }
    }

    #[test]
    fn ledger_template_has_the_required_fields() {
        // The validator requires id, tier_shape, source_url and retrieved.
        for field in [
            "id:",
            "tier_shape:",
            "floor_severity:",
            "source_url:",
            "retrieved:",
        ] {
            assert!(
                LEDGER_TEMPLATE.contains(field),
                "ledger template is missing {field}"
            );
        }
    }

    #[test]
    fn ledger_template_does_not_offer_a_removed_shape() {
        // `any` was removed from the taxonomy: a program documenting no severity
        // floor is low_and_above. The template must not list it as an option, or
        // every scaffolded ledger starts with a value the validator rejects.
        assert!(
            !LEDGER_TEMPLATE.contains("| any |"),
            "ledger template still lists `any` as a shape option"
        );
        assert!(
            LEDGER_TEMPLATE.contains("low_and_above"),
            "ledger template must offer low_and_above"
        );
        // And it should explain what to do with a no-floor program, so the
        // researcher is not left guessing. The template must NOT tell them to
        // write low_and_above: an unstated floor is not evidence that Low pays.
        assert!(
            LEDGER_TEMPLATE.contains("floor_status: unstated"),
            "ledger template should tell the researcher to mark an unstated floor"
        );
        assert!(
            LEDGER_TEMPLATE.contains("NO STATED floor"),
            "ledger template should explain the no-floor case"
        );
        assert!(
            LEDGER_TEMPLATE.contains("not evidence that Low pays"),
            "ledger template should say why an unstated floor is not a Low floor"
        );
    }

    #[test]
    fn known_issues_template_uses_the_parsed_shape() {
        // `read_known_issues` accepts a mapping with a `known_issues:` key.
        assert!(KNOWN_ISSUES_TEMPLATE.contains("known_issues:"));
        for field in ["id:", "summary:", "source:", "patterns:"] {
            assert!(
                KNOWN_ISSUES_TEMPLATE.contains(field),
                "known-issues template is missing {field}"
            );
        }
    }

    #[test]
    fn history_template_uses_the_parsed_shape() {
        // read_log accepts a mapping with a submissions: key.
        assert!(HISTORY_TEMPLATE.contains("submissions:"));
        for field in ["family:", "outcome:", "note:", "date:"] {
            assert!(
                HISTORY_TEMPLATE.contains(field),
                "history template is missing {field}"
            );
        }
    }

    #[test]
    fn history_template_says_it_is_local() {
        // The one file the tool writes that contains the author's own bug
        // families must say, in the file, that it is not for publication.
        let low = HISTORY_TEMPLATE.to_lowercase();
        assert!(
            low.contains("never publish"),
            "history template must say so"
        );
    }

    #[test]
    fn yaml_quote_escapes_and_quotes_when_needed() {
        assert_eq!(yaml_quote("plain words here"), "plain words here");
        assert_eq!(yaml_quote("has: colon"), "\"has: colon\"");
        assert_eq!(yaml_quote("has \"quote\""), "\"has \\\"quote\\\"\"");
        assert_eq!(yaml_quote(""), "\"\"");
    }

    #[test]
    fn today_iso_has_the_expected_shape() {
        let s = today_iso();
        assert_eq!(s.len(), 10, "{s}");
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        let year: i32 = s[..4].parse().expect("year parses");
        assert!((2020..2100).contains(&year), "{s}");
    }

    #[test]
    fn log_round_trips_through_the_parser() {
        let dir = std::env::temp_dir().join("vg-log-roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("history.yaml");
        log_add(
            &path,
            "stale oracle bound",
            "duplicate",
            Some("note: with a colon"),
            Some("2026-10-10"),
        )
        .expect("write");
        log_add(&path, "second family", "paid", None, Some("2026-10-11")).expect("append");
        let text = std::fs::read_to_string(&path).unwrap();
        let entries = parse_log(&text).expect("parse");
        assert_eq!(entries.len(), 2, "{text}");
        assert_eq!(entries[0].family, "stale oracle bound");
        assert_eq!(entries[0].note.as_deref(), Some("note: with a colon"));
        assert_eq!(entries[1].outcome, "paid");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scope_template_is_comment_only() {
        // A scope list with a stray non-comment line would silently enter the
        // gate as a real path.
        for line in SCOPE_TEMPLATE.lines() {
            let t = line.trim();
            assert!(
                t.is_empty() || t.starts_with('#'),
                "scope template has a non-comment line: {t:?}"
            );
        }
    }
}
