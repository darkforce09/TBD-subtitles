use super::*;

#[test]
fn every_required_model_is_pinned_in_the_manifest() {
    let pinned = inference::model_store::manifest::model_ids();
    for separator in [Separator::Roformer, Separator::MdxNet] {
        for whisper in [WhisperModel::LargeV3, WhisperModel::LargeV3Turbo] {
            let mut settings = JobSettings::with_glossary(vec![]);
            settings.separator = separator;
            settings.whisper = whisper;
            for model in required(&settings) {
                assert!(pinned.contains(&model), "{model} is not in the manifest");
            }
        }
    }
}

#[test]
fn the_settings_choose_the_separator_and_whisper_folders() {
    let mut settings = JobSettings::with_glossary(vec![]);
    settings.separator = Separator::MdxNet;
    settings.whisper = WhisperModel::LargeV3Turbo;
    let models = required(&settings);
    assert_eq!(models.len(), 10);
    assert!(models.contains(&"lama-inpaint") && models.contains(&"latin-fonts"));
    assert!(models.contains(&"pp-ocrv5"));
    assert!(models.contains(&"manga-ocr"));
    assert!(models.contains(&"qwen3.5-4b"));
    assert!(models.contains(&"mdx-net-voc-ft"));
    assert!(models.contains(&"whisper-large-v3-turbo"));
    assert!(!models.contains(&"whisper-large-v3"));
}

#[test]
fn an_empty_folder_misses_every_required_model() {
    let settings = JobSettings::with_glossary(vec![]);
    let empty = std::env::temp_dir().join(format!("tbd-models-empty-{}", std::process::id()));
    assert_eq!(missing(&empty, &settings), required(&settings));
}
