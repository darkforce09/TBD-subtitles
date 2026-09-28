//! The report logic: reading a finished job's files into a `JobReport` or a row's summary, and
//! counting its lines worth a listen and its problems, with no rendering code.

pub(crate) mod line_counts;
pub(crate) mod report_loading;
