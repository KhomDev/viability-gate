//! Human-readable and JSON rendering.

use crate::model::*;

const RULE: &str = "────────────────────────────────────────────────────────────";

pub fn text(report: &Report) -> String {
    let mut out = String::new();

    out.push_str(&format!(
        "{} {}  —  {}\n\n",
        report.tool, report.version, report.subject
    ));
    out.push_str(&format!("  OUTCOME   {}\n", report.outcome.word()));
    out.push_str(&format!("            {}\n", report.outcome.meaning()));
    out.push_str(&format!(
        "  SCORE     {} / 100   (uncalibrated heuristic; not a probability; threshold {})\n\n",
        report.viability_score, report.threshold
    ));

    if report.findings.is_empty() {
        out.push_str("  No anti-pattern fired.\n\n");
    } else {
        for f in &report.findings {
            let marker = match f.verdict {
                Verdict::Hard => "HARD",
                Verdict::Soft => "SOFT",
                Verdict::Warn => "NOTE",
            };
            out.push_str(&format!("  {marker}  {:<4} {}\n", f.rule, f.title));
            for e in &f.evidence {
                out.push_str(&format!("             ↳ matched: {e}\n"));
            }
            for s in &f.matched_sentences {
                out.push_str(&format!("             ↳ in sentence: {s}\n"));
            }
            for line in wrap(&f.fix, 74) {
                out.push_str(&format!("             → {line}\n"));
            }
            out.push_str(&format!(
                "             ({}; evidence: {}; origin rows: {})\n\n",
                f.doc,
                f.evidence_kind.label(),
                f.origin_rows
            ));
        }
    }

    if !report.passed.is_empty() {
        out.push_str(&format!("  PASS  {}\n", report.passed.join(", ")));
    }

    if !report.warnings.is_empty() {
        out.push('\n');
        for w in &report.warnings {
            for (i, line) in wrap(&format!("input: {w}"), 70).into_iter().enumerate() {
                if i == 0 {
                    out.push_str(&format!("  NOTE  {line}\n"));
                } else {
                    out.push_str(&format!("        {line}\n"));
                }
            }
        }
    }

    let unknown: Vec<&GateOutcome> = report
        .gates
        .iter()
        .filter(|g| g.status == GateStatus::Unknown)
        .collect();
    if !unknown.is_empty() {
        let parts: Vec<String> = unknown
            .iter()
            .map(|g| format!("{} ({})", g.gate, first_clause(&g.detail)))
            .collect();
        out.push_str(&format!("  SKIP  {}\n", parts.join("   ")));
    }
    out.push('\n');

    if !report.gates.is_empty() {
        let marks: Vec<String> = report
            .gates
            .iter()
            .map(|g| {
                let sym = match g.status {
                    GateStatus::Pass => "✓",
                    GateStatus::Fail => "✗",
                    GateStatus::Unknown => "?",
                };
                format!("{} {sym}", g.gate)
            })
            .collect();
        out.push_str(&format!("  Gates:  {}\n\n", marks.join("   ")));
    }

    out.push_str(&format!("  {RULE}\n"));
    for line in wrap(&report.disclaimer, 74) {
        out.push_str(&format!("  {line}\n"));
    }
    out
}

pub fn gates_text(outcomes: &[GateOutcome], verbose: bool) -> String {
    let mut out = String::new();
    for g in outcomes {
        let sym = match g.status {
            GateStatus::Pass => "PASS",
            GateStatus::Fail => "FAIL",
            GateStatus::Unknown => " ?  ",
        };
        out.push_str(&format!("  {sym}  {:<4} {}\n", g.gate, g.title));
        for line in wrap(&format!("Q: {}", g.question), 70) {
            out.push_str(&format!("           {line}\n"));
        }
        out.push_str(&format!("           needs: {}\n", g.needs));
        for line in wrap(&g.detail, 70) {
            out.push_str(&format!("           {line}\n"));
        }
        out.push_str(&format!(
            "           evidence: {} ({} row(s))\n",
            g.evidence.label(),
            g.origin_rows
        ));
        if verbose {
            for line in wrap(&format!("blocks: {}", g.blocks), 68) {
                out.push_str(&format!("           {line}\n"));
            }
            out.push_str(&format!("           weight: {} of 100\n", g.weight));
            for c in &g.checks {
                for (i, line) in wrap(c, 66).into_iter().enumerate() {
                    if i == 0 {
                        out.push_str(&format!("           check: {line}\n"));
                    } else {
                        out.push_str(&format!("                  {line}\n"));
                    }
                }
            }
        }
        out.push('\n');
    }
    out
}

/// Collapse a detail string to its first clause, for the one-line SKIP summary.
fn first_clause(s: &str) -> String {
    s.split(" - ").next().unwrap_or(s).trim().to_string()
}

/// Wrap text to the given width on word boundaries.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub fn json(report: &Report) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

pub fn rules_list(rules: &[Rule]) -> String {
    let mut out = String::new();
    let mut group = String::new();
    for r in rules {
        if r.group != group {
            group = r.group.clone();
            let name = match group.as_str() {
                "A" => "A — the impact doesn't exist",
                "B" => "B — the precondition is unreachable or disallowed",
                "C" => "C — the harm is bounded or unprofitable",
                "D" => "D — already known",
                "E" => "E — the bug is fine; the report fails",
                other => other,
            };
            out.push_str(&format!("\n{name}\n"));
        }
        out.push_str(&format!(
            "  {:<4} {:<9} {}\n",
            r.id,
            format!("{:?}", r.verdict).to_lowercase(),
            r.title
        ));
    }
    out
}

pub fn explain(rule: &Rule) -> String {
    let mut out = String::new();
    out.push_str(&format!("{} — {}\n\n", rule.id, rule.title));
    out.push_str(&format!("  group     {}\n", rule.group));
    out.push_str(&format!("  verdict   {:?}\n", rule.verdict).to_lowercase());
    out.push_str(&format!("  weight    {} of 100\n", rule.weight));
    out.push_str(&format!(
        "  evidence  {} ({} row(s) of the author's log)\n",
        rule.evidence.label(),
        rule.origin_rows
    ));
    out.push_str(&format!(
        "  fp_rate   {}\n",
        match rule.fp_rate {
            Some(r) => format!("{r}"),
            None => "UNMEASURED - no accepted/paid control set exists yet".to_string(),
        }
    ));
    out.push_str(&format!(
        "  validated {}\n\n",
        rule.last_validated.as_deref().unwrap_or("never")
    ));
    out.push_str("  detection\n");
    if let Some(kind) = rule.structural.as_deref() {
        out.push_str(&format!("    structural matcher: {kind}\n"));
    }
    if !rule.any_of.is_empty() {
        for p in &rule.any_of {
            out.push_str(&format!("    any of: {p}\n"));
        }
    }
    if !rule.all_of.is_empty() {
        for p in &rule.all_of {
            out.push_str(&format!("    all of: {p}\n"));
        }
    }
    if !rule.none_of.is_empty() {
        for p in &rule.none_of {
            out.push_str(&format!("    suppressed by: {p}\n"));
        }
        out.push_str(&format!("    suppression scope: {}\n", rule.none_of_scope));
    }
    if !rule.escalate_on.is_empty() {
        for p in &rule.escalate_on {
            out.push_str(&format!("    escalates to hard on: {p}\n"));
        }
    }
    if !rule.downgrade_on.is_empty() {
        for p in &rule.downgrade_on {
            out.push_str(&format!("    ignored (too weak) when: {p}\n"));
        }
    }
    if !rule.ignore_sections.is_empty() {
        out.push_str(&format!(
            "    ignores sections: {}\n",
            rule.ignore_sections.join(", ")
        ));
    }
    out.push_str(&format!(
        "    negation window: {}\n",
        if rule.negation_aware { "on" } else { "off" }
    ));
    out.push_str("\n  fix\n");
    for line in wrap(&rule.fix, 72) {
        out.push_str(&format!("    {line}\n"));
    }
    out.push_str(&format!("\n  docs  {}\n", rule.doc));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_respects_width() {
        let lines = wrap("aaa bbb ccc ddd", 7);
        assert_eq!(lines, vec!["aaa bbb", "ccc ddd"]);
    }

    #[test]
    fn wrap_handles_long_word() {
        let lines = wrap("supercalifragilistic", 5);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn first_clause_truncates() {
        assert_eq!(
            first_clause("no scope tree supplied - unknown, not pass"),
            "no scope tree supplied"
        );
    }
}
