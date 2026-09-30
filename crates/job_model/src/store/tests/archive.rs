use super::TableLayouts;
use crate::archive_round_trip::round_trip;

#[test]
fn table_layouts_round_trip() {
    round_trip(&TableLayouts {
        versions: [("frames", 2), ("meta", 1), ("step_records", 1)]
            .into_iter()
            .map(|(name, version)| (name.to_string(), version))
            .collect(),
    });
    round_trip(&TableLayouts::default());
}
