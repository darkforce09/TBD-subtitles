use std::path::PathBuf;

use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::*;
use crate::core::ui::theme;
use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::page::SignLibrary;

fn page(library: SignLibrary) -> SettingsPage {
    SettingsPage {
        path: PathBuf::from("/settings/settings.toml"),
        saved: AppSettings::default(),
        error: None,
        unreadable: None,
        glossary_names: None,
        items: Vec::new(),
        download: None,
        downloaded_at: None,
        checks: None,
        checking: false,
        models_folder: PathBuf::from("/models"),
        models_size: None,
        work_folder: PathBuf::from("/work"),
        work_size: None,
        right_click: Default::default(),
        library,
    }
}

/// The On-screen Text tab of `page`. The first frame only installs the window's fonts, which the
/// icons need from the next frame on.
fn harness(page: &SettingsPage) -> Harness<'_, Vec<SettingsEvent>> {
    let mut installed = false;
    let mut harness = Harness::builder()
        .with_size(vec2(660.0, 1600.0))
        .build_ui_state(
            move |ui, events: &mut Vec<SettingsEvent>| {
                if !installed {
                    theme::install(ui.ctx());
                    installed = true;
                    return;
                }
                onscreen_text_ui(ui, page, events);
            },
            Vec::new(),
        );
    harness.run();
    harness
}

#[test]
fn the_sign_library_shows_its_size_and_clear_asks_first() {
    let page = page(SignLibrary {
        size: Some((3, 2 * 1_048_576)),
        ..SignLibrary::default()
    });
    let mut harness = harness(&page);
    harness.get_by_label("3 signs, 2.0 MiB.");
    harness.get_by_label(LIBRARY_HELP);
    assert!(harness.state().is_empty(), "an idle frame asks for nothing");
    harness.get_by_label("Clear…").click();
    harness.run();
    assert_eq!(harness.state().as_slice(), [SettingsEvent::AskClearLibrary]);
}

#[test]
fn a_confirmed_clear_clears_and_cancel_keeps_the_library() {
    let page = page(SignLibrary {
        size: Some((3, 4096)),
        confirming: true,
        ..SignLibrary::default()
    });
    let mut harness = harness(&page);
    harness.get_by_label("Remove all 3 signs?");
    harness.get_by_label("Clear").click();
    harness.run();
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(
        harness.state().as_slice(),
        [SettingsEvent::ClearLibrary, SettingsEvent::KeepLibrary]
    );
}

#[test]
fn an_empty_or_unmeasured_library_offers_nothing_to_clear() {
    for library in [
        SignLibrary {
            size: Some((0, 0)),
            ..SignLibrary::default()
        },
        SignLibrary::default(),
    ] {
        let page = page(library);
        let mut harness = harness(&page);
        harness.get_by_label("Clear…").click();
        harness.run();
        assert!(harness.state().is_empty());
    }
    let page = page(SignLibrary {
        error: Some("sign library: cannot open it".into()),
        ..SignLibrary::default()
    });
    let harness = harness(&page);
    harness.get_by_label("Measuring…");
    harness.get_by_label("sign library: cannot open it");
}

#[test]
fn the_engine_encoder_and_decoder_choices_each_send_one_edit() {
    let page = page(SignLibrary::default());
    let mut harness = harness(&page);
    harness.get_by_label("TensorRT").click();
    harness.run();
    harness.get_by_label("NVENC (GPU)").click();
    harness.run();
    harness.get_by_label("Decode video on the GPU").click();
    harness.run();
    let edits: Vec<(DetectorEngine, LocalizedEncoder, bool)> = harness
        .state()
        .iter()
        .map(|event| match event {
            SettingsEvent::Edit(edited) => (
                edited.onscreen_text.detector_engine,
                edited.onscreen_text.localized_encoder,
                edited.onscreen_text.hardware_decode,
            ),
            _ => panic!("only edits"),
        })
        .collect();
    assert_eq!(
        edits,
        [
            (DetectorEngine::TensorRt, LocalizedEncoder::X264, false),
            (DetectorEngine::Cuda, LocalizedEncoder::Nvenc, false),
            (DetectorEngine::Cuda, LocalizedEncoder::X264, true),
        ]
    );
}
