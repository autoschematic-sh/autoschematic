// use std::{collections::HashSet, path::PathBuf};

// use anyhow::bail;


// use super::types::{PlanReport, PlanReportSet};

// TODO this was a half-baked attempt at getting an entire plan to run with no deferrals and plugged/templated outputs mid-plan/apply.
// For reasons of severe complexity, and for the simple fact that a connector may not know whether a plan produces connector
// op A or connector op B until it has the actual values in its hand, this approach has been left out.
//
// pub fn solve_plan_report_set_schedule(mut report_set: PlanReportSet) -> anyhow::Result<()> {
//     let mut scheduled: Vec<usize> = Vec::new();
//     let mut deferred: Vec<usize> = (0..report_set.plan_reports.len()).collect();
//     let mut next_deferred = Vec::new();

//     let mut outputs_pending_write = HashSet::<(PathBuf, String)>::new();

//     // First phase: pretend outputs on-disk don't exist at
//     // all, to eagerly schedule ops that take outputs from earlier plans.
//     loop {
//         'report: for report_i in &deferred {
//             // For each output the report reads, 
//             // check if it's pending a write by a scheduled report.
//             // If neither, defer it.
//             // Else, schedule it.
//             //
//             let report: &PlanReport = &report_set.plan_reports[*report_i];

//             let prefix = report.prefix.clone();
//             let virt_addr = report.virt_addr.clone();
//             'output: for output in &report.reads_outputs {
//                 if !outputs_pending_write.contains(&(
//                     PathBuf::from(prefix.clone()).join(&output.path),
//                     output.key.clone(),
//                 )) {
//                     next_deferred.push(*report_i);
//                     continue 'report;
//                 }
//             }

//             for connector_op in &report.connector_ops {
//                 for output in &connector_op.writes_outputs {
//                     outputs_pending_write.insert((
//                         PathBuf::from(prefix.clone()).join(virt_addr.clone()),
//                         output.to_string(),
//                     ));
//                 }
//             }

//             scheduled.push(*report_i);
//         }

//         if next_deferred.len() == 0 {
//             break;
//         }

//         if deferred == next_deferred {
//             bail!("Cycle found")
//         }

//         deferred = next_deferred.clone();
//     }

//     // loop {
//     //     'report: for report_i in &deferred {
//     //         // For each output the report reads, first check if it already exists on-disk.
//     //         // If not, check if it's pending a write by a scheduled report.
//     //         // If neither, defer it.
//     //         // Else, schedule it.
//     //         //
//     //         let report: &PlanReport = &report_set.plan_reports[*report_i];

//     //         let prefix = report.prefix.clone();
//     //         let virt_addr = report.virt_addr.clone();
//     //         'output: for output in &report.reads_outputs {
//     //             if load_read_output(PathBuf::from(prefix.clone()), &output)?.is_none()
//     //                 && !outputs_pending_write.contains(&(
//     //                     PathBuf::from(prefix.clone()).join(&output.path),
//     //                     output.key.clone(),
//     //                 ))
//     //             {
//     //                 next_deferred.push(*report_i);
//     //                 continue 'report;
//     //             }
//     //         }

//     //         for connector_op in &report.connector_ops {
//     //             for output in &connector_op.writes_outputs {
//     //                 outputs_pending_write.insert((
//     //                     PathBuf::from(prefix.clone()).join(virt_addr.clone()),
//     //                     output.to_string(),
//     //                 ));
//     //             }
//     //         }

//     //         scheduled.push(*report_i);
//     //     }

//     //     if deferred == next_deferred {
//     //         break;
//     //     }

//     //     deferred = next_deferred.clone();
//     // }

//     report_set.plan_reports = scheduled.into_iter().map(|i| report_set.plan_reports[i].clone()).collect::<Vec<PlanReport>>();

//     // report_set.plan_reports = scheduled;
//     Ok(())
// }
