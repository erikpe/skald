//! Human and canonical JSON projections of one typed report.

use std::fmt::Write;

use crate::model::{CandidateCounts, MeasurementReport, ReportFormat};

pub fn render_report(report: &MeasurementReport, format: ReportFormat) -> Result<String, String> {
    match format {
        ReportFormat::Json => serde_json::to_string_pretty(report)
            .map(|mut output| {
                output.push('\n');
                output
            })
            .map_err(|error| format!("could not serialize measurement report: {error}")),
        ReportFormat::Human => Ok(render_human(report)),
    }
}

fn render_human(report: &MeasurementReport) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "local final-MIR redundancy: {} v{}",
        report.corpus.name, report.corpus.version
    );
    let _ = writeln!(
        output,
        "compiler: {}{}",
        report.compiler.revision,
        if report.compiler.dirty {
            " (dirty)"
        } else {
            ""
        }
    );
    let schedule = report
        .schedule
        .iter()
        .map(|occurrence| format!("{}#{}", occurrence.pass, occurrence.occurrence))
        .collect::<Vec<_>>()
        .join(" -> ");
    let _ = writeln!(output, "schedule: {schedule}");
    let _ = writeln!(output, "workloads: {}", report.workloads.len());
    for workload in &report.workloads {
        let _ = writeln!(output, "\n{} [{}]", workload.id, workload.category);
        for snapshot in &workload.snapshots {
            let _ = writeln!(
                output,
                "  {:<16} spill {}  casts {}  cse {}  instructions {}",
                snapshot.name,
                compact(&snapshot.scalar_spill),
                compact(&snapshot.redundant_casts),
                compact(&snapshot.local_cse),
                snapshot.structure.instructions
            );
        }
    }
    if !report.totals.snapshots.is_empty() {
        let _ = writeln!(output, "\ntotals");
        for snapshot in &report.totals.snapshots {
            let _ = writeln!(
                output,
                "  {:<16} spill {}  casts {}  cse {}  instructions {}",
                snapshot.name,
                compact(&snapshot.scalar_spill),
                compact(&snapshot.redundant_casts),
                compact(&snapshot.local_cse),
                snapshot.structure.instructions
            );
        }
    }
    output
}

fn compact(counts: &CandidateCounts) -> String {
    format!("{}/{}", counts.proven, counts.interesting,)
}

#[cfg(test)]
mod tests {
    use super::render_report;
    use crate::model::{
        CompilerIdentity, Configuration, CorpusIdentity, MeasurementReport, ReportFormat, Totals,
    };

    fn empty_report() -> MeasurementReport {
        MeasurementReport {
            schema: 1,
            corpus: CorpusIdentity {
                name: "test".to_owned(),
                version: 1,
            },
            compiler: CompilerIdentity {
                revision: "abc".to_owned(),
                dirty: false,
            },
            configuration: Configuration {
                target: "x86_64-sysv",
                runtime_trace: "omitted",
                mir_profile: "default",
                mir_exclusions: Vec::new(),
            },
            schedule: Vec::new(),
            workloads: Vec::new(),
            totals: Totals::default(),
        }
    }

    #[test]
    fn human_and_json_project_the_same_typed_identity() {
        let report = empty_report();
        let human = render_report(&report, ReportFormat::Human).unwrap();
        let json = render_report(&report, ReportFormat::Json).unwrap();
        assert!(human.contains("test v1"));
        assert!(human.contains("compiler: abc"));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["schema"], 1);
        assert_eq!(value["corpus"]["name"], "test");
        assert!(json.find("\"schema\"").unwrap() < json.find("\"corpus\"").unwrap());
        assert!(json.find("\"corpus\"").unwrap() < json.find("\"compiler\"").unwrap());
        assert!(json.find("\"compiler\"").unwrap() < json.find("\"configuration\"").unwrap());
        assert!(json.ends_with('\n'));
    }
}
