//! vg bench - measure the catalog against a labelled set of findings.
//!
//! This is the measurement harness. It reports, per rule:
//!
//!   * fire rate on the REJECTED set - how often the rule fires on a finding
//!     that really was rejected
//!   * fire rate and FALSE-KILL rate on the ACCEPTED set - how often a rule
//!     fires (and hard-fails) a finding that was accepted or paid
//!
//! The false-KILL rate on an accepted set is the number that matters, and it is
//! the number this tool has never been able to measure, because the author's own
//! log contains no paid findings. See docs/control-set.md for how to build one
//! from public paid reports, and docs/prospective-protocol.md for the frozen,
//! pre-registered test that would settle it.
//!
//! NO NUMBER HERE IS A VALIDATION. It is a measurement of the tool against
//! whatever set you point it at. A small synthetic fixture set measures nothing
//! about the world; it only proves the harness runs.

use crate::engine::CompiledRule;
use crate::model::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Serialize, Default)]
pub struct RuleStat {
    pub rule: String,
    pub verdict: String,
    /// Findings in the rejected set this rule fired on.
    pub rejected_fires: usize,
    /// Findings in the accepted set this rule fired on.
    pub accepted_fires: usize,
    /// Accepted findings this rule HARD-failed. This is a false KILL.
    pub accepted_hard_kills: usize,
    pub rejected_total: usize,
    pub accepted_total: usize,
}

impl RuleStat {
    pub fn rejected_rate(&self) -> f64 {
        rate(self.rejected_fires, self.rejected_total)
    }
    pub fn accepted_rate(&self) -> f64 {
        rate(self.accepted_fires, self.accepted_total)
    }
    pub fn false_kill_rate(&self) -> f64 {
        rate(self.accepted_hard_kills, self.accepted_total)
    }
}

fn rate(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 / d as f64
    }
}

#[derive(Debug, Serialize, Default)]
pub struct BenchReport {
    pub rejected_total: usize,
    pub accepted_total: usize,
    /// Present only when a labels CSV was supplied.
    pub labelled: bool,
    pub per_rule: Vec<RuleStat>,
    /// Overall: rejected findings on which at least one hard rule fired.
    pub rejected_hard_kills: usize,
    /// Overall: accepted findings on which at least one hard rule fired.
    pub accepted_hard_kills: usize,
    pub notes: Vec<String>,
}

pub fn run(
    rules: &[CompiledRule],
    rejected: &[(String, String)],
    accepted: &[(String, String)],
    labels: Option<&Path>,
) -> Result<BenchReport, String> {
    let mut report = BenchReport {
        rejected_total: rejected.len(),
        accepted_total: accepted.len(),
        labelled: false,
        ..Default::default()
    };

    if let Some(path) = labels {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut known = 0usize;
        for line in text.lines().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            let mut parts = line.split(',');
            let name = parts.next().unwrap_or("").trim();
            if rejected.iter().any(|(n, _)| n == name) || accepted.iter().any(|(n, _)| n == name) {
                known += 1;
            }
        }
        report.labelled = true;
        report.notes.push(format!(
            "labels CSV supplied: {known} label row(s) matched a finding filename"
        ));
    }

    let mut stats: BTreeMap<String, RuleStat> = BTreeMap::new();
    for cr in rules {
        stats.insert(
            cr.rule.id.clone(),
            RuleStat {
                rule: cr.rule.id.clone(),
                verdict: cr.rule.verdict.label().to_string(),
                rejected_total: rejected.len(),
                accepted_total: accepted.len(),
                ..Default::default()
            },
        );
    }

    for (_, text) in rejected {
        let (findings, _) = crate::engine::evaluate(rules, text);
        let mut any_hard = false;
        for f in &findings {
            if let Some(s) = stats.get_mut(&f.rule) {
                s.rejected_fires += 1;
            }
            if f.verdict == Verdict::Hard {
                any_hard = true;
            }
        }
        if any_hard {
            report.rejected_hard_kills += 1;
        }
    }

    for (_, text) in accepted {
        let (findings, _) = crate::engine::evaluate(rules, text);
        let mut any_hard = false;
        for f in &findings {
            if let Some(s) = stats.get_mut(&f.rule) {
                s.accepted_fires += 1;
                if f.verdict == Verdict::Hard {
                    s.accepted_hard_kills += 1;
                }
            }
            if f.verdict == Verdict::Hard {
                any_hard = true;
            }
        }
        if any_hard {
            report.accepted_hard_kills += 1;
        }
    }

    report.per_rule = stats.into_values().collect();
    if report.accepted_total == 0 {
        report.notes.push(
            "no accepted set supplied: the false-KILL rate is UNMEASURED, and this report \
             says so rather than reporting zero"
                .to_string(),
        );
    }
    Ok(report)
}

pub fn to_text(report: &BenchReport) -> String {
    let mut out = String::new();
    out.push_str("vg bench\n\n");
    out.push_str(&format!(
        "  rejected findings: {}   accepted findings: {}\n",
        report.rejected_total, report.accepted_total
    ));
    out.push_str(&format!(
        "  hard-KILL on rejected: {}/{}    hard-KILL on accepted: {}/{} (false kills)\n\n",
        report.rejected_hard_kills,
        report.rejected_total,
        report.accepted_hard_kills,
        report.accepted_total
    ));
    out.push_str(&format!(
        "  {:<5} {:<6} {:>12} {:>12} {:>14}\n",
        "RULE", "VERDICT", "REJ FIRE", "ACC FIRE", "ACC FALSE-KILL"
    ));
    for s in &report.per_rule {
        out.push_str(&format!(
            "  {:<5} {:<6} {:>12} {:>12} {:>14}\n",
            s.rule,
            s.verdict,
            format!("{}/{}", s.rejected_fires, s.rejected_total),
            format!("{}/{}", s.accepted_fires, s.accepted_total),
            format!("{}/{}", s.accepted_hard_kills, s.accepted_total),
        ));
    }
    out.push_str(&format!(
        "\n  {:<5} {:<6} {:>12} {:>12} {:>14}\n",
        "", "", "rate", "rate", "rate"
    ));
    for s in &report.per_rule {
        out.push_str(&format!(
            "  {:<5} {:<6} {:>11.1}% {:>11.1}% {:>13.1}%\n",
            s.rule,
            s.verdict,
            100.0 * s.rejected_rate(),
            100.0 * s.accepted_rate(),
            100.0 * s.false_kill_rate(),
        ));
    }
    out.push('\n');
    for n in &report.notes {
        out.push_str(&format!("  NOTE  {n}\n"));
    }
    out.push_str(
        "\n  These are measurements against the set you supplied, not a validation of the \
         tool. A rule that fires on nothing is dead weight; a rule that hard-fails an \
         accepted finding is a false kill and matters more than any other number here.\n",
    );
    out
}

pub fn to_json(report: &BenchReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_default()
}
