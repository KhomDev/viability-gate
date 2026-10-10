//! Matching engine: run the catalog against a finding, evaluate the gates, and
//! compute an outcome.
//!
//! The engine is deterministic and offline. It never contacts the network, and it
//! holds no target data - every target-specific input arrives at runtime.
//!
//! MATCHING SEMANTICS (0.2.0)
//! --------------------------
//! 1. A document is split into SECTIONS at markdown headings, and each section
//!    into SENTENCES.
//! 2. any_of / all_of are evaluated sentence by sentence.
//! 3. A match is discarded when a negation cue appears within the N tokens
//!    immediately before it (default 6). Rules that already handle negation
//!    through none_of set negation_aware: false so the two mechanisms do not
//!    fight.
//! 4. Every fire reports the matched snippet AND its sentence, so the user can
//!    see exactly what the tool reacted to and argue with it.
//!
//! Everything here is a heuristic over prose. It cannot verify a claim in the
//! report, and it can be gamed by wording. See docs/rules/ for the per-rule
//! false-positive modes.

use crate::model::*;
use regex::Regex;

/// Tokens that negate a following match, within the negation window.
const NEGATION_CUES: [&str; 18] = [
    "no", "not", "never", "without", "neither", "nor", "cannot", "cant", "none", "nobody",
    "nothing", "lack", "lacks", "lacking", "absent", "fails", "fail", "unable",
];

/// How many tokens before a match the negation window looks at.
pub const NEGATION_WINDOW: usize = 6;

/// A rule with its regexes pre-compiled, so matching is a single pass.
pub struct CompiledRule {
    pub rule: Rule,
    any_of: Vec<Regex>,
    all_of: Vec<Regex>,
    none_of: Vec<Regex>,
    escalate_on: Vec<Regex>,
    downgrade_on: Vec<Regex>,
}

#[derive(Debug)]
pub enum RuleError {
    BadRegex {
        rule: String,
        pattern: String,
        error: String,
    },
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleError::BadRegex {
                rule,
                pattern,
                error,
            } => {
                write!(f, "rule {rule}: invalid regex {pattern:?}: {error}")
            }
        }
    }
}

fn compile(rule_id: &str, patterns: &[String]) -> Result<Vec<Regex>, RuleError> {
    patterns
        .iter()
        .map(|p| {
            Regex::new(&format!("(?i){p}")).map_err(|e| RuleError::BadRegex {
                rule: rule_id.to_string(),
                pattern: p.clone(),
                error: e.to_string(),
            })
        })
        .collect()
}

impl CompiledRule {
    pub fn new(rule: Rule) -> Result<Self, RuleError> {
        let any_of = compile(&rule.id, &rule.any_of)?;
        let all_of = compile(&rule.id, &rule.all_of)?;
        let none_of = compile(&rule.id, &rule.none_of)?;
        let escalate_on = compile(&rule.id, &rule.escalate_on)?;
        let downgrade_on = compile(&rule.id, &rule.downgrade_on)?;
        Ok(Self {
            rule,
            any_of,
            all_of,
            none_of,
            escalate_on,
            downgrade_on,
        })
    }

    /// Does this rule fire against the text?
    pub fn matches(&self, text: &str) -> Option<Match> {
        // A structural rule ignores the generic matcher entirely.
        if let Some(kind) = self.rule.structural.as_deref() {
            return structural_match(kind, text);
        }

        let sections = split_sections(text);

        // Document-scoped escape hatch. A none_of hit anywhere suppresses the
        // rule, unless the rule scopes suppression to the sentence.
        let sentence_scoped_none = self.rule.none_of_scope == "sentence";
        if !sentence_scoped_none {
            for re in &self.none_of {
                if re.is_match(text) {
                    return None;
                }
            }
        }

        let mut hits: Vec<SentenceHit> = Vec::new();
        let mut strong: Vec<SentenceHit> = Vec::new();

        for section in &sections {
            if self.rule.ignore_sections.iter().any(|s| s == &section.kind) {
                continue;
            }
            for sentence in &section.sentences {
                if sentence_scoped_none && self.none_of.iter().any(|re| re.is_match(sentence.text))
                {
                    continue;
                }
                for re in &self.any_of {
                    if let Some(hit) = find_unnegated(re, sentence, self.rule.negation_aware) {
                        if self.downgrade_on.iter().any(|d| d.is_match(&hit.snippet)) {
                            continue;
                        }
                        hits.push(hit);
                    }
                }
                for re in &self.escalate_on {
                    if let Some(hit) = find_unnegated(re, sentence, self.rule.negation_aware) {
                        strong.push(hit);
                    }
                }
            }
        }

        if !self.any_of.is_empty() && hits.is_empty() && strong.is_empty() {
            return None;
        }

        // all_of must also hold, each pattern somewhere, non-negated.
        for re in &self.all_of {
            let mut found = false;
            for section in &sections {
                if self.rule.ignore_sections.iter().any(|s| s == &section.kind) {
                    continue;
                }
                for sentence in &section.sentences {
                    if find_unnegated(re, sentence, self.rule.negation_aware).is_some() {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            if !found {
                return None;
            }
        }

        // An all_of-only rule draws its evidence from the all_of patterns.
        if self.any_of.is_empty() && hits.is_empty() && strong.is_empty() {
            for re in &self.all_of {
                for section in &sections {
                    if self.rule.ignore_sections.iter().any(|s| s == &section.kind) {
                        continue;
                    }
                    for sentence in &section.sentences {
                        if let Some(hit) = find_unnegated(re, sentence, self.rule.negation_aware) {
                            hits.push(hit);
                        }
                    }
                }
            }
        }

        hits.extend(strong);
        hits.dedup_by(|a, b| a.snippet == b.snippet && a.sentence == b.sentence);
        if hits.is_empty() {
            return None;
        }
        hits.truncate(3);

        let verdict = self.effective_verdict(text);
        Some(Match {
            evidence: hits.iter().map(|h| h.snippet.clone()).collect(),
            sentences: hits.iter().map(|h| h.sentence.clone()).collect(),
            verdict,
        })
    }

    /// The verdict this rule actually carries on this document.
    ///
    /// escalate_on: when a strong form is present, the rule carries its full
    /// weight even if its base verdict is softer.
    fn effective_verdict(&self, text: &str) -> Verdict {
        let strong = self
            .escalate_on
            .iter()
            .any(|re| any_unnegated(re, text, self.rule.negation_aware));
        if strong {
            return self.rule.escalate_to.unwrap_or(Verdict::Hard);
        }
        self.rule.verdict
    }
}

#[derive(Debug, Clone)]
pub struct Match {
    pub evidence: Vec<String>,
    pub sentences: Vec<String>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone)]
struct SentenceHit {
    snippet: String,
    sentence: String,
}

#[derive(Debug)]
struct Sentence<'a> {
    text: &'a str,
}

#[derive(Debug)]
struct Section<'a> {
    kind: String,
    sentences: Vec<Sentence<'a>>,
}

/// Classify a section heading. Only the kinds the rules ask about are named.
fn section_kind(heading: &str) -> String {
    let low = heading.to_lowercase();
    let has = |needle: &str| low.contains(needle);
    if has("proof of concept") || has("poc") || has("reproduc") || has("exploit") || has("test") {
        "poc".to_string()
    } else if has("impact")
        || has("severity")
        || has("headline")
        || has("summary")
        || has("description")
    {
        "claim".to_string()
    } else {
        "other".to_string()
    }
}

/// Split a document into sections at markdown headings, then into sentences.
fn split_sections(text: &str) -> Vec<Section<'_>> {
    let mut sections: Vec<Section<'_>> = Vec::new();
    let mut current_kind = "claim".to_string();
    let mut current_start = 0usize;

    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            if offset > current_start {
                sections.push(Section {
                    kind: current_kind.clone(),
                    sentences: split_sentences(&text[current_start..offset]),
                });
            }
            let heading = trimmed.trim_start_matches('#').trim();
            current_kind = section_kind(heading);
            // Start the new section AT the heading, so the headline text is part
            // of the section it introduces. A finding's headline is its top heading;
            // discarding it hid the claim that P19 exists to compare against the proof.
            current_start = offset;
        }
        offset += line.len();
    }
    if current_start < text.len() {
        sections.push(Section {
            kind: current_kind,
            sentences: split_sentences(&text[current_start..]),
        });
    }
    if sections.is_empty() {
        sections.push(Section {
            kind: "claim".to_string(),
            sentences: split_sentences(text),
        });
    }
    sections
}

/// Split a block into sentences on terminal punctuation and newlines.
fn split_sentences(text: &str) -> Vec<Sentence<'_>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        let terminal = c == b'.' || c == b'!' || c == b'?' || c == b'\n';
        if terminal {
            // A '.' inside a number is not a boundary.
            let next_is_digit = c == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit();
            let prev_is_digit = c == b'.' && i > 0 && bytes[i - 1].is_ascii_digit();
            if !(next_is_digit || prev_is_digit) {
                let slice = &text[start..i];
                if !slice.trim().is_empty() {
                    out.push(Sentence { text: slice });
                }
                start = i + 1;
            }
        }
        i += 1;
    }
    if start < text.len() {
        let slice = &text[start..];
        if !slice.trim().is_empty() {
            out.push(Sentence { text: slice });
        }
    }
    out
}

/// Find the pattern in the sentence, rejecting a match whose negation window
/// holds a cue.
fn find_unnegated(
    re: &Regex,
    sentence: &Sentence<'_>,
    negation_aware: bool,
) -> Option<SentenceHit> {
    for m in re.find_iter(sentence.text) {
        if negation_aware && is_negated(sentence.text, m.start()) {
            continue;
        }
        return Some(SentenceHit {
            snippet: collapse(m.as_str()),
            sentence: collapse(sentence.text),
        });
    }
    None
}

fn any_unnegated(re: &Regex, text: &str, negation_aware: bool) -> bool {
    for section in split_sections(text) {
        for sentence in &section.sentences {
            if find_unnegated(re, sentence, negation_aware).is_some() {
                return true;
            }
        }
    }
    false
}

/// Is the match negated by a cue in the preceding window?
fn is_negated(sentence: &str, at: usize) -> bool {
    let prefix = &sentence[..at];
    let tokens: Vec<String> = prefix
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect();
    let start = tokens.len().saturating_sub(NEGATION_WINDOW);
    tokens[start..]
        .iter()
        .any(|t| NEGATION_CUES.contains(&t.as_str()) || t.ends_with("n't"))
}

fn collapse(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = one.chars().take(200).collect();
    if one.chars().count() > 200 {
        out.push('…');
    }
    out
}

// ------------------------------------------------------- structural rules

/// Patterns that assert something persisted AFTER the failure.
const PERSISTED_STATE: [&str; 8] = [
    "persist",
    "storage",
    "state remains",
    "after the call returns",
    "remains after",
    "committed",
    "unchanged across",
    "survives the",
];

const LOSS_CLAIM: &str = r"(?i)(permanent|irreversible|unrecoverable|total|complete|100%)\s+(\w+\s+){0,3}(lock|loss|freeze|brick|strand|drain|theft|dead)";
const FAILURE_ONLY: &str = r"(?i)\b(reverts?|reverted|panics?|aborts?|expected_failure|assertion fail|div(ision)? by zero)\b";

/// P19, redesigned.
///
/// The old form fired whenever a loss claim and a revert co-occurred ANYWHERE.
/// That is also how a valid revert-based freeze is described, so it killed
/// findings it should have passed. This form is structural:
///
///   * the HEADLINE claims permanent or total loss, AND
///   * the PoC/proof section shows ONLY a revert/panic/expected failure, AND
///   * the document asserts no persisted state.
///
/// When those hold, the proof disproves the title: that is the SPAM boundary,
/// and the rule is hard. Otherwise the rule still fires, but as a NOTE - the
/// claim and the evidence disagree and a human should look.
fn structural_match(kind: &str, text: &str) -> Option<Match> {
    if kind != "p19_headline_vs_proof" {
        return None;
    }
    let sections = split_sections(text);
    let claim_re = Regex::new(LOSS_CLAIM).ok()?;
    let fail_re = Regex::new(FAILURE_ONLY).ok()?;

    let mut headline_hit: Option<String> = None;
    let mut poc_text = String::new();
    let mut seen_poc = false;
    for section in &sections {
        if section.kind == "poc" {
            seen_poc = true;
        }
        for sentence in &section.sentences {
            if !seen_poc {
                if headline_hit.is_none() && claim_re.is_match(sentence.text) {
                    headline_hit = Some(collapse(sentence.text));
                }
            } else {
                poc_text.push_str(sentence.text);
                poc_text.push(' ');
            }
        }
    }

    let headline = headline_hit?;

    // No failure shown in the proof means there is no contradiction to report:
    // the rule stays silent rather than firing on the headline alone.
    let poc_shows_failure = fail_re.is_match(&poc_text);
    if !poc_shows_failure {
        return None;
    }

    let persisted_asserted = PERSISTED_STATE
        .iter()
        .any(|p| text.to_lowercase().contains(p));

    // A persisted-state assertion is exactly what would make the headline TRUE.
    // Without one, the proof disproves the title and this is the SPAM boundary.
    let verdict = if persisted_asserted {
        Verdict::Warn
    } else {
        Verdict::Hard
    };

    Some(Match {
        evidence: vec![headline.clone()],
        sentences: vec![headline],
        verdict,
    })
}

// ---------------------------------------------------------------- evaluate

/// Run every rule against the finding text.
pub fn evaluate(rules: &[CompiledRule], text: &str) -> (Vec<Finding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut passed = Vec::new();

    for cr in rules {
        match cr.matches(text) {
            Some(m) => findings.push(Finding {
                rule: cr.rule.id.clone(),
                title: cr.rule.title.clone(),
                group: cr.rule.group.clone(),
                verdict: m.verdict,
                weight: cr.rule.weight,
                evidence: m.evidence,
                matched_sentences: m.sentences,
                fix: cr.rule.fix.clone(),
                doc: cr.rule.doc.clone(),
                evidence_kind: cr.rule.evidence,
                origin_rows: cr.rule.origin_rows,
            }),
            None => passed.push(cr.rule.id.clone()),
        }
    }
    (findings, passed)
}

/// Compute the report outcome and the viability score.
///
/// ONE decision rule, in this order:
///   1. any fired rule with a hard verdict   -> KILL
///   2. any fired rule with a soft verdict   -> HOLD
///   3. score under the threshold            -> HOLD
///   4. any fired rule with a warn verdict   -> NOTE
///   5. any gate Unknown                     -> INCOMPLETE
///   6. otherwise                            -> CLEAR
///
/// The score is 100 minus the hand-set weights of the fired rules, clamped to
/// 0..=100. It is an UNCALIBRATED HEURISTIC and not a probability. Gates do NOT
/// deduct from it; a gate that did not run produces INCOMPLETE instead.
pub fn outcome(findings: &[Finding], gates_unknown: bool, threshold: u32) -> (Outcome, u32) {
    let mut score: i32 = 100;
    for f in findings {
        score -= f.weight as i32;
    }
    let score = score.clamp(0, 100) as u32;

    let has_hard = findings.iter().any(|f| f.verdict == Verdict::Hard);
    let has_soft = findings.iter().any(|f| f.verdict == Verdict::Soft);
    let has_warn = findings.iter().any(|f| f.verdict == Verdict::Warn);

    let outcome = if has_hard {
        Outcome::Kill
    } else if has_soft || score < threshold {
        Outcome::Hold
    } else if has_warn {
        Outcome::Note
    } else if gates_unknown {
        Outcome::Incomplete
    } else {
        Outcome::Clear
    };
    (outcome, score)
}

// ---------------------------------------------------------------- gates

/// Evaluate the pre-hunt gates from whatever inputs were supplied.
///
/// A gate whose input is missing reports Unknown, never Pass. Unknown is not
/// pass: a gate you could not run is a gate you have not run.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_gates(
    gates: &[Gate],
    ledger: Option<&[LedgerEntry]>,
    scope: Option<&[String]>,
    prior_audits: Option<&[String]>,
    tests: Option<&str>,
    fork_of: Option<&str>,
    own_log: Option<&[LogEntry]>,
    finding_text: Option<&str>,
) -> Vec<GateOutcome> {
    gates
        .iter()
        .map(|g| {
            let (status, detail) = match g.id.as_str() {
                "G1" => gate_floor(ledger),
                "G2" => gate_exclusions(ledger),
                "G3" => gate_scope(scope),
                "G4" => gate_prior_audits(prior_audits, ledger),
                "G5" => gate_tests(tests),
                "G6" => gate_fork(fork_of),
                "G7" => gate_own_history(own_log, finding_text),
                "D1" => (
                    GateStatus::Unknown,
                    "D1 is a corpus match - supply the program's known issues with \
                     --known-issues"
                        .to_string(),
                ),
                _ => (GateStatus::Unknown, "unrecognised gate".to_string()),
            };
            GateOutcome {
                gate: g.id.clone(),
                title: g.title.clone(),
                question: g.question.clone(),
                needs: g.needs.clone(),
                blocks: g.blocks.clone(),
                weight: g.weight,
                status,
                detail,
                checks: g
                    .checks
                    .iter()
                    .map(|c| format!("{} — {} (fail: {})", c.name, c.test, c.fail))
                    .collect(),
                evidence: g.evidence,
                origin_rows: g.origin_rows,
            }
        })
        .collect()
}

fn gate_floor(ledger: Option<&[LedgerEntry]>) -> (GateStatus, String) {
    let Some(entries) = ledger else {
        return (
            GateStatus::Unknown,
            "no ledger supplied - cannot check the payout floor".into(),
        );
    };
    if entries.is_empty() {
        return (GateStatus::Unknown, "ledger is empty".into());
    }
    let mut notes = Vec::new();
    let mut worst = GateStatus::Pass;
    for e in entries {
        // An uncited floor is a guess, and a guessed floor is worse than none
        // because you will act on it.
        match (e.source_url.as_deref(), e.retrieved.as_deref()) {
            (None, _) | (Some(""), _) => {
                worst = GateStatus::Fail;
                notes.push(format!("{}: no source_url - uncited floor", e.id));
            }
            (_, None) => {
                worst = GateStatus::Fail;
                notes.push(format!("{}: no retrieved date - uncited floor", e.id));
            }
            (Some(_), Some(date)) => {
                if let Some(days) = days_since(date) {
                    if days > 180 {
                        notes.push(format!(
                            "{}: retrieved {days} days ago - re-read the page, terms change",
                            e.id
                        ));
                    }
                }
            }
        }

        // An UNSTATED floor is not a Low floor. It is a page that has not been
        // read, and it must not be read as permissive.
        match e.floor_status.as_deref() {
            Some("unstated") => {
                worst = GateStatus::Fail;
                notes.push(format!(
                    "{}: floor_status 'unstated' - no floor is documented. That is not evidence \
                     that Low pays; it is evidence the page has not been read",
                    e.id
                ));
            }
            Some("conflicting") => {
                worst = GateStatus::Fail;
                notes.push(format!(
                    "{}: floor_status 'conflicting' - the program's own materials disagree. \
                     The reward table governs; resolve it before hunting",
                    e.id
                ));
            }
            Some("documented") | None => {}
            Some(other) => {
                worst = GateStatus::Fail;
                notes.push(format!("{}: unknown floor_status {other:?}", e.id));
            }
        }

        match e.tier_shape.as_deref() {
            None => {
                worst = GateStatus::Fail;
                notes.push(format!("{}: tier_shape missing", e.id));
            }
            Some("unknown") => {
                worst = GateStatus::Fail;
                notes.push(format!(
                    "{}: tier_shape 'unknown' - NOT permissive, the page has not been read",
                    e.id
                ));
            }
            Some(shape @ ("critical_only" | "high_and_above")) => {
                worst = GateStatus::Fail;
                notes.push(format!(
                    "{}: tier_shape '{shape}' pays nothing below that bar{}",
                    e.id,
                    e.floor_severity
                        .as_deref()
                        .map(|f| format!(" (floor: {f})"))
                        .unwrap_or_default()
                ));
            }
            Some(shape) => notes.push(format!(
                "{}: tier_shape '{shape}'{}",
                e.id,
                e.floor_severity
                    .as_deref()
                    .map(|f| format!(" (floor: {f})"))
                    .unwrap_or_default()
            )),
        }
        for c in &e.conflicts {
            notes.push(format!("{}: CONFLICT - {c}", e.id));
        }
    }
    (worst, notes.join("; "))
}

/// Days between an ISO YYYY-MM-DD date and today, computed from the Unix epoch
/// without pulling in a date library.
fn days_since(iso: &str) -> Option<i64> {
    let mut parts = iso.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days_from_civil = |y: i64, m: i64, d: i64| -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    };
    let then = days_from_civil(y, m, d);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64
        / 86_400;
    Some(now - then)
}

fn gate_exclusions(ledger: Option<&[LedgerEntry]>) -> (GateStatus, String) {
    let Some(entries) = ledger else {
        return (
            GateStatus::Unknown,
            "no ledger supplied - cannot check exclusions".into(),
        );
    };
    let mut total = 0usize;
    let mut problems = Vec::new();
    for e in entries {
        if e.exclusions.is_empty() {
            problems.push(format!("{}: no exclusion clauses transcribed", e.id));
            continue;
        }
        for (i, x) in e.exclusions.iter().enumerate() {
            total += 1;
            if x.clause.as_deref().unwrap_or("").trim().is_empty() {
                problems.push(format!("{}: exclusion[{i}] missing verbatim clause", e.id));
            }
            if let Some(p) = &x.pattern {
                if Regex::new(&format!("(?i){p}")).is_err() {
                    problems.push(format!("{}: exclusion[{i}] invalid regex", e.id));
                }
            }
            let _ = &x.class;
        }
    }
    if !problems.is_empty() {
        return (GateStatus::Fail, problems.join("; "));
    }
    if total == 0 {
        return (GateStatus::Fail, "no exclusion clauses transcribed".into());
    }
    let mut classes: Vec<&str> = entries
        .iter()
        .flat_map(|e| e.exclusions.iter())
        .map(|x| x.class.as_str())
        .collect();
    classes.sort_unstable();
    classes.dedup();
    let delta = entries
        .iter()
        .filter_map(|e| e.post_audit_delta.as_deref())
        .next()
        .map(|d| format!("; post-audit delta recorded: {d}"))
        .unwrap_or_default();
    (
        GateStatus::Pass,
        format!(
            "{total} clause(s) transcribed and well-formed across {} class(es){delta}",
            classes.len()
        ),
    )
}

fn gate_scope(scope: Option<&[String]>) -> (GateStatus, String) {
    match scope {
        None => (
            GateStatus::Unknown,
            "no scope tree supplied - unknown, not pass".into(),
        ),
        Some([]) => (GateStatus::Fail, "scope list is empty".into()),
        Some(s) => (GateStatus::Pass, format!("{} scoped path(s)", s.len())),
    }
}

fn gate_prior_audits(
    prior: Option<&[String]>,
    ledger: Option<&[LedgerEntry]>,
) -> (GateStatus, String) {
    let mut notes = Vec::new();
    let mut supplied = 0usize;
    if let Some(entries) = ledger {
        for e in entries {
            for (i, a) in e.prior_audits.iter().enumerate() {
                if a.firm.is_none() {
                    notes.push(format!("{}: prior_audits[{i}] has no firm name", e.id));
                }
                if a.url.is_none() {
                    notes.push(format!("{}: prior_audits[{i}] has no url", e.id));
                }
                if a.covers.is_empty() {
                    notes.push(format!(
                        "{}: prior_audits[{i}] has no 'covers' list - read the finding, not the contents page",
                        e.id
                    ));
                } else {
                    supplied += a.covers.len();
                }
                for k in &a.known_issues {
                    notes.push(format!("{}: known issue - {k}", e.id));
                }
            }
        }
    }
    match prior {
        None => {
            let mut detail = "no prior-audit surface supplied - unknown, not pass".to_string();
            if !notes.is_empty() {
                detail.push_str("; ");
                detail.push_str(&notes.join("; "));
            }
            (GateStatus::Unknown, detail)
        }
        Some(p) if p.is_empty() && supplied == 0 => {
            (GateStatus::Fail, "prior-audit list is empty".into())
        }
        Some(p) => {
            if notes.iter().any(|n| n.contains("no 'covers' list")) {
                (GateStatus::Fail, notes.join("; "))
            } else {
                (
                    GateStatus::Pass,
                    format!(
                        "{} prior-artifact(s) supplied, {} path(s) covered",
                        p.len(),
                        supplied
                    ),
                )
            }
        }
    }
}

fn gate_tests(tests: Option<&str>) -> (GateStatus, String) {
    let Some(text) = tests else {
        return (
            GateStatus::Unknown,
            "no test suite supplied - unknown, not pass".into(),
        );
    };
    const MARKERS: [&str; 6] = [
        "by design",
        "should preserve",
        "intended",
        "expected to survive",
        "not penalised",
        "documented behavior",
    ];
    let low = text.to_lowercase();
    let hits: Vec<&str> = MARKERS
        .iter()
        .copied()
        .filter(|m| low.contains(m))
        .collect();
    if hits.is_empty() {
        (
            GateStatus::Pass,
            "no intent markers found in the suite".into(),
        )
    } else {
        (
            GateStatus::Fail,
            format!(
                "intent markers present ({}). Read the test before submitting - this is Not Applicable, not Informational",
                hits.join(", ")
            ),
        )
    }
}

fn gate_fork(fork_of: Option<&str>) -> (GateStatus, String) {
    match fork_of {
        None => (
            GateStatus::Unknown,
            "fork status unknown - unknown, not pass".into(),
        ),
        Some("none") | Some("no") | Some("") => (GateStatus::Pass, "not a fork".into()),
        Some(name) => (
            GateStatus::Fail,
            format!(
                "fork of {name} - check whether the behaviour exists identically upstream and is documented there"
            ),
        ),
    }
}

/// G7 - the author's own submission history.
///
/// Fails closed: with no log supplied, this reports Unknown, never Pass. The
/// reason this gate exists at all is that a regex over a finding's prose cannot
/// see the researcher's own earlier submissions, and in the source log 12 of 97
/// rows repeat a family already present.
fn gate_own_history(log: Option<&[LogEntry]>, finding: Option<&str>) -> (GateStatus, String) {
    let Some(entries) = log else {
        return (
            GateStatus::Unknown,
            "no own-history log supplied - unknown, not pass. Run vg init to scaffold one".into(),
        );
    };
    if entries.is_empty() {
        return (
            GateStatus::Fail,
            "own-history log is empty - no submissions recorded".into(),
        );
    }
    let Some(text) = finding else {
        return (
            GateStatus::Unknown,
            format!(
                "{} logged submission(s); no finding text supplied to compare against",
                entries.len()
            ),
        );
    };
    let matches = probable_repeats(entries, text);
    if matches.is_empty() {
        (
            GateStatus::Pass,
            format!(
                "{} logged submission(s); no probable repeat above the similarity threshold",
                entries.len()
            ),
        )
    } else {
        let names: Vec<String> = matches
            .iter()
            .map(|(e, score)| format!("{} ({:.2})", e.family, score))
            .collect();
        (
            GateStatus::Fail,
            format!(
                "probable repeat of {} earlier submission(s): {}. Read the earlier outcome before submitting",
                matches.len(),
                names.join(", ")
            ),
        )
    }
}

/// Deterministic token-overlap similarity between a finding and a logged family.
///
/// Jaccard over lowercased alphanumeric tokens of length >= 4, minus a small
/// stop list. No model, no network, no randomness: the same inputs always give
/// the same answer, and the threshold is documented in the README.
pub const REPEAT_THRESHOLD: f64 = 0.18;

pub fn probable_repeats<'a>(entries: &'a [LogEntry], text: &str) -> Vec<(&'a LogEntry, f64)> {
    let finding = tokens(text);
    if finding.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(&LogEntry, f64)> = entries
        .iter()
        .filter_map(|e| {
            let mut probe = e.family.clone();
            if let Some(n) = &e.note {
                probe.push(' ');
                probe.push_str(n);
            }
            let logged = tokens(&probe);
            if logged.is_empty() {
                return None;
            }
            let inter = finding.intersection(&logged).count() as f64;
            let union = finding.union(&logged).count() as f64;
            let j = if union == 0.0 { 0.0 } else { inter / union };
            if j >= REPEAT_THRESHOLD {
                Some((e, j))
            } else {
                None
            }
        })
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

const STOP: [&str; 12] = [
    "that", "this", "with", "from", "when", "then", "than", "into", "over", "have", "been", "which",
];

fn tokens(text: &str) -> std::collections::BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 4)
        .map(|t| t.to_lowercase())
        .filter(|t| !STOP.contains(&t.as_str()))
        .collect()
}

// ---------------------------------------------------------------- known issues

/// Match a finding against a user-supplied known-issue corpus.
///
/// This is a KNOWN-ISSUE MATCHER, not a duplicate catcher. It can only see what
/// the user already transcribed, so it CANNOT catch a concurrent duplicate: a
/// concurrent duplicate is, by definition, not in any corpus yet.
///
/// A match is reported as a KI finding with a hard verdict, because a known
/// issue is Not Applicable rather than Informational - a materially worse
/// outcome than an unpaid finding.
pub fn match_known_issues(issues: &[KnownIssue], text: &str) -> (Vec<Finding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut warnings = Vec::new();

    for issue in issues {
        if issue.patterns.is_empty() {
            warnings.push(format!(
                "{}: no patterns, so it cannot be matched - transcribe a regex",
                issue.id
            ));
            continue;
        }
        let mut evidence: Vec<String> = Vec::new();
        let mut sentences: Vec<String> = Vec::new();
        for pattern in &issue.patterns {
            let re = match Regex::new(&format!("(?i){pattern}")) {
                Ok(re) => re,
                Err(e) => {
                    warnings.push(format!("{}: invalid regex {pattern:?}: {e}", issue.id));
                    continue;
                }
            };
            for section in split_sections(text) {
                let mut hit_here = false;
                for sentence in &section.sentences {
                    if let Some(hit) = find_unnegated(&re, sentence, true) {
                        evidence.push(hit.snippet);
                        sentences.push(hit.sentence);
                        hit_here = true;
                        break;
                    }
                }
                if hit_here {
                    break;
                }
            }
            if evidence.len() >= 2 {
                break;
            }
        }
        if !evidence.is_empty() {
            findings.push(Finding {
                rule: format!("KI:{}", issue.id),
                title: format!("Matches known issue - {}", issue.summary),
                group: "K".into(),
                verdict: Verdict::Hard,
                weight: 60,
                evidence,
                matched_sentences: sentences,
                fix: "Do not submit. A known issue is Not Applicable, not Informational - a \
                      materially worse outcome than an unpaid finding. Read the referenced \
                      source and confirm the root cause is the same, not merely adjacent."
                    .into(),
                doc: issue
                    .source
                    .clone()
                    .unwrap_or_else(|| "user-supplied known-issues corpus".into()),
                evidence_kind: Evidence::None,
                origin_rows: 0,
            });
        }
    }
    (findings, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, verdict: Verdict, any_of: &[&str], none_of: &[&str]) -> CompiledRule {
        CompiledRule::new(Rule {
            id: id.into(),
            title: "t".into(),
            group: "A".into(),
            verdict,
            weight: 30,
            any_of: any_of.iter().map(|s| s.to_string()).collect(),
            all_of: vec![],
            none_of: none_of.iter().map(|s| s.to_string()).collect(),
            escalate_on: vec![],
            downgrade_on: vec![],
            escalate_to: None,
            structural: None,
            none_of_scope: "document".into(),
            ignore_sections: vec![],
            fix: "f".into(),
            doc: "d".into(),
            origin_rows: 0,
            evidence: Evidence::None,
            last_validated: None,
            fp_rate: None,
            negation_aware: true,
        })
        .unwrap()
    }

    #[test]
    fn fires_on_match() {
        let r = rule("P1", Verdict::Hard, &["not currently"], &[]);
        assert!(r.matches("This is not currently exploitable.").is_some());
    }

    #[test]
    fn does_not_fire_without_match() {
        let r = rule("P1", Verdict::Hard, &["not currently"], &[]);
        assert!(r.matches("The attacker drains the vault.").is_none());
    }

    #[test]
    fn none_of_suppresses() {
        let r = rule("P3", Verdict::Hard, &["front[- ]?run"], &["atomic"]);
        assert!(r.matches("The attacker front-runs the operator.").is_some());
        assert!(r
            .matches("Front-run, but atomic in one transaction.")
            .is_none());
    }

    #[test]
    fn negation_window_discards_a_negated_match() {
        let r = rule("P3", Verdict::Hard, &["front[- ]?run"], &[]);
        // The cue is inside the window.
        assert!(r
            .matches("There is no front-run exposure in this path.")
            .is_none());
        // The cue is outside the window (more than six tokens back).
        let far = "There is no realistic scenario at all in which the attacker can \
                   practically arrange the state such that they front-run the operator.";
        assert!(r.matches(far).is_some(), "distant cue must not suppress");
    }

    #[test]
    fn negation_aware_false_ignores_the_window() {
        let mut r = rule("P3", Verdict::Hard, &["front[- ]?run"], &[]);
        r.rule.negation_aware = false;
        assert!(r
            .matches("There is no front-run exposure in this path.")
            .is_some());
    }

    #[test]
    fn sentence_scoped_none_of_only_suppresses_that_sentence() {
        let mut r = rule("P2", Verdict::Hard, &["admin can"], &["permissionless"]);
        r.rule.none_of_scope = "sentence".into();
        // The escape hatch is in a DIFFERENT sentence, so the rule still fires.
        let text = "An admin can set the fee to zero. The path is permissionless for anyone else.";
        assert!(r.matches(text).is_some());
        // Same sentence: suppressed.
        let text = "An admin can set the fee to zero and the path is permissionless.";
        assert!(r.matches(text).is_none());
    }

    #[test]
    fn evidence_reports_the_snippet_and_its_sentence() {
        let r = rule("P7", Verdict::Soft, &["best[- ]practice"], &[]);
        let m = r
            .matches("Some preamble. This is a best-practice critique of the code. Trailing.")
            .expect("must fire");
        assert_eq!(m.evidence.len(), 1);
        assert!(m.evidence[0].contains("best-practice"));
        assert!(
            m.sentences[0].contains("critique of the code"),
            "the sentence must be reported: {:?}",
            m.sentences
        );
    }

    #[test]
    fn all_of_requires_every_pattern() {
        let mut r = rule("P19", Verdict::Hard, &["permanent fund lock"], &[]);
        r.all_of = vec![Regex::new("(?i)\\breverts?\\b").unwrap()];
        assert!(r
            .matches("Permanent fund lock because it reverts.")
            .is_some());
        assert!(r
            .matches("Permanent fund lock with no failure mode.")
            .is_none());
    }

    #[test]
    fn downgrade_on_drops_a_weak_match() {
        let mut r = rule("P4", Verdict::Hard, &["Mock[A-Z]\\w*"], &[]);
        r.downgrade_on = vec![Regex::new("(?i)MockERC20").unwrap()];
        // A bare scaffolding mock must not fire the rule at all.
        assert!(r.matches("The PoC uses MockERC20 as the token.").is_none());
        assert!(r
            .matches("The PoC uses MockVault instead of the real contract.")
            .is_some());
    }

    #[test]
    fn escalate_on_raises_the_verdict() {
        let mut r = rule("P4", Verdict::Warn, &["Mock[A-Z]\\w*"], &[]);
        r.escalate_on = vec![Regex::new("(?i)instead of the real").unwrap()];
        r.rule.escalate_to = Some(Verdict::Hard);
        let m = r
            .matches("The PoC uses MockVault instead of the real contract.")
            .expect("must fire");
        assert_eq!(m.verdict, Verdict::Hard);
        let m = r
            .matches("The PoC uses MockVault somewhere.")
            .expect("must still fire as a note");
        assert_eq!(m.verdict, Verdict::Warn);
    }

    #[test]
    fn ignore_sections_skips_the_poc() {
        let mut r = rule("P5", Verdict::Hard, &["outside the scope"], &[]);
        r.rule.ignore_sections = vec!["poc".into()];
        let text = "# Finding\n\nThe root cause is in scope.\n\n## Proof of concept\n\nThis is outside the scope.\n";
        assert!(r.matches(text).is_none(), "the PoC section must be ignored");
    }

    // ------------------------------------------------------ structural P19

    #[test]
    fn p19_fires_hard_when_the_proof_disproves_the_title() {
        let r = structural_rule();
        let text = "# Permanent fund lock in the validator\n\n## Proof of concept\n\nThe transaction reverts and all state is rolled back.\n";
        let m = r.matches(text).expect("must fire");
        assert_eq!(m.verdict, Verdict::Hard, "the PoC disproves the title");
    }

    #[test]
    fn p19_downgrades_when_a_persisted_consequence_is_asserted() {
        let r = structural_rule();
        let text = "# Permanent fund lock in the validator\n\n## Proof of concept\n\nThe call reverts for the attacker, but the committed storage slot keeps the funds stranded after the call returns.\n";
        let m = r.matches(text).expect("must still fire");
        assert_eq!(
            m.verdict,
            Verdict::Warn,
            "a persisted consequence means the claim may be true"
        );
    }

    #[test]
    fn p19_does_not_fire_on_a_loss_claim_with_no_failure_in_the_proof() {
        let r = structural_rule();
        let text = "# Permanent fund lock in the validator\n\n## Proof of concept\n\nThe attacker withdraws the entire balance to their own address.\n";
        assert!(r.matches(text).is_none(), "no revert means no mismatch");
    }

    fn structural_rule() -> CompiledRule {
        let mut r = rule("P19", Verdict::Hard, &[], &[]);
        r.rule.structural = Some("p19_headline_vs_proof".into());
        r
    }

    // ------------------------------------------------------ outcome

    fn finding(verdict: Verdict, weight: u32) -> Finding {
        Finding {
            rule: "X".into(),
            title: "t".into(),
            group: "A".into(),
            verdict,
            weight,
            evidence: vec![],
            matched_sentences: vec![],
            fix: String::new(),
            doc: String::new(),
            evidence_kind: Evidence::None,
            origin_rows: 0,
        }
    }

    #[test]
    fn hard_fail_kills_even_at_a_high_score() {
        let (o, score) = outcome(&[finding(Verdict::Hard, 50)], false, 50);
        assert_eq!(o, Outcome::Kill);
        assert_eq!(score, 50);
    }

    #[test]
    fn a_clean_finding_with_a_known_gate_is_clear() {
        let (o, score) = outcome(&[], false, 50);
        assert_eq!(o, Outcome::Clear);
        assert_eq!(score, 100);
    }

    #[test]
    fn unknown_gate_with_no_findings_is_incomplete_not_clear() {
        let (o, _) = outcome(&[], true, 50);
        assert_eq!(o, Outcome::Incomplete);
    }

    #[test]
    fn warn_only_is_note_not_clear() {
        let (o, _) = outcome(&[finding(Verdict::Warn, 10)], false, 50);
        assert_eq!(o, Outcome::Note);
    }

    #[test]
    fn score_below_threshold_holds() {
        let (o, _) = outcome(&[finding(Verdict::Warn, 60)], false, 50);
        assert_eq!(o, Outcome::Hold);
    }

    #[test]
    fn no_outcome_word_is_submit() {
        for o in [
            Outcome::Kill,
            Outcome::Hold,
            Outcome::Note,
            Outcome::Incomplete,
            Outcome::Clear,
        ] {
            assert_ne!(o.word(), "SUBMIT");
        }
    }

    #[test]
    fn gate_reports_unknown_not_pass_without_input() {
        let gates = vec![Gate {
            id: "G3".into(),
            title: "Scope tree".into(),
            question: "q".into(),
            needs: "n".into(),
            blocks: "b".into(),
            weight: 30,
            checks: vec![],
            evidence: Evidence::None,
            origin_rows: 0,
            implemented_by: None,
            run_by_gates_command: true,
        }];
        let out = evaluate_gates(&gates, None, None, None, None, None, None, None);
        assert_eq!(out[0].status, GateStatus::Unknown);
    }

    #[test]
    fn g7_fails_closed_without_a_log() {
        let gates = vec![Gate {
            id: "G7".into(),
            title: "Own history".into(),
            question: "q".into(),
            needs: "n".into(),
            blocks: "b".into(),
            weight: 25,
            checks: vec![],
            evidence: Evidence::None,
            origin_rows: 0,
            implemented_by: None,
            run_by_gates_command: true,
        }];
        let out = evaluate_gates(&gates, None, None, None, None, None, None, Some("text"));
        assert_eq!(out[0].status, GateStatus::Unknown);
    }

    #[test]
    fn g7_flags_a_probable_repeat() {
        let log = vec![LogEntry {
            family: "stale oracle price bound".into(),
            outcome: "duplicate".into(),
            note: Some("cross-vault withdrawal replay".into()),
            date: None,
        }];
        let gates = vec![Gate {
            id: "G7".into(),
            title: "Own history".into(),
            question: "q".into(),
            needs: "n".into(),
            blocks: "b".into(),
            weight: 25,
            checks: vec![],
            evidence: Evidence::None,
            origin_rows: 0,
            implemented_by: None,
            run_by_gates_command: true,
        }];
        let text = "A stale oracle price bound lets the attacker replay a withdrawal.";
        let out = evaluate_gates(&gates, None, None, None, None, None, Some(&log), Some(text));
        assert_eq!(out[0].status, GateStatus::Fail);
        assert!(out[0].detail.contains("probable repeat"));
    }

    #[test]
    fn unstated_floor_is_a_failure_not_permissive() {
        let ledger = vec![LedgerEntry {
            id: "prog".into(),
            tier_shape: Some("unknown".into()),
            floor_severity: None,
            floor_status: Some("unstated".into()),
            source_url: Some("https://example.org/rewards".into()),
            retrieved: Some("2026-10-01".into()),
            conflicts: vec![],
            exclusions: vec![],
            prior_audits: vec![],
            post_audit_delta: None,
        }];
        let (status, detail) = gate_floor(Some(&ledger));
        assert_eq!(status, GateStatus::Fail);
        assert!(detail.contains("unstated"));
    }

    // ------------------------------------------------------ known issues

    fn ki(id: &str, patterns: &[&str]) -> KnownIssue {
        KnownIssue {
            id: id.into(),
            summary: "a previously accepted behaviour".into(),
            patterns: patterns.iter().map(|s| s.to_string()).collect(),
            source: Some("prior audit ref".into()),
        }
    }

    #[test]
    fn known_issue_matches_and_is_hard() {
        let issues = vec![ki("KI-1", &["burned amount", "re-?lock"])];
        let (findings, warnings) = match_known_issues(
            &issues,
            "The burned amount persists across unlock and re-lock cycles.",
        );
        assert_eq!(findings.len(), 1, "expected a match");
        assert_eq!(findings[0].verdict, Verdict::Hard);
        assert!(findings[0].rule.starts_with("KI:"));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn known_issue_does_not_match_unrelated_text() {
        let issues = vec![ki("KI-1", &["burned amount"])];
        let (findings, _) =
            match_known_issues(&issues, "A signed order is replayable across vaults.");
        assert!(findings.is_empty(), "must not fire on unrelated prose");
    }

    #[test]
    fn known_issue_without_patterns_warns_rather_than_silently_passing() {
        let issues = vec![ki("KI-2", &[])];
        let (findings, warnings) = match_known_issues(&issues, "anything at all");
        assert!(findings.is_empty());
        assert_eq!(warnings.len(), 1, "an unmatchable entry must be reported");
        assert!(warnings[0].contains("KI-2"));
    }

    #[test]
    fn known_issue_invalid_regex_warns() {
        let issues = vec![ki("KI-3", &["(unclosed"])];
        let (findings, warnings) = match_known_issues(&issues, "some finding text");
        assert!(findings.is_empty());
        assert!(
            warnings.iter().any(|w| w.contains("invalid regex")),
            "{warnings:?}"
        );
    }
}
