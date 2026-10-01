use std::time::Duration;

use super::*;
use crate::library::fixtures::{Scratch, library_sign};

fn library(scratch: &Scratch) -> Library {
    Library::at(scratch.path().join("data").join(FILE_NAME))
}

#[test]
fn a_library_with_no_file_holds_nothing_and_creates_nothing() {
    let scratch = Scratch::new("empty");
    let library = library(&scratch);
    assert_eq!(library.lookup("海軍", 0).unwrap(), None);
    assert_eq!(library.remove("海軍", 0).unwrap(), 0);
    assert_eq!(library.size().unwrap(), LibrarySize::default());
    library.clear().unwrap();
    assert!(!library.path().exists());
}

#[test]
fn a_recorded_sign_is_found_by_its_normalised_text_and_a_near_crop_hash() {
    let scratch = Scratch::new("lookup");
    let library = library(&scratch);
    let hash = 0x00ff_00ff_00ff_00ff;
    assert!(
        library
            .record(library_sign("海軍 本部", hash, "Navy HQ", "d11"))
            .unwrap()
    );
    let near = hash ^ 0b11_1111;
    let found = library.lookup("海軍本部", near).unwrap().expect("a match");
    assert_eq!(found.english, "Navy HQ");
    assert_eq!(found.crop_hash, hash);
    assert_eq!(library.lookup("海軍本部", hash ^ 0b111_1111).unwrap(), None);
    assert_eq!(library.lookup("海軍", hash).unwrap(), None);
    let size = library.size().unwrap();
    assert_eq!(size.signs, 1);
    assert!(size.bytes > 0);
}

#[test]
fn a_lookup_skips_the_signs_of_the_asking_job_and_takes_the_nearest() {
    let scratch = Scratch::new("nearest");
    let library = library(&scratch);
    let hash = 0x1234_5678_9abc_def0;
    library
        .record_all(vec![
            library_sign("王宮", hash, "Royal Palace", "d11"),
            library_sign("王宮", hash ^ 0xff00_0000_0000_0000, "The Palace", "d12"),
        ])
        .unwrap();
    let asked = [("王宮", hash ^ 0b1)];
    let from = |job| library.lookup_all(&asked, Some(job)).unwrap()[0].clone();
    assert_eq!(from("d28").unwrap().english, "Royal Palace");
    assert_eq!(from("d11"), None);
    let far = [("王宮", hash ^ 0xff00_0000_0000_0001)];
    let found = library.lookup_all(&far, Some("d11")).unwrap();
    assert_eq!(found[0].as_ref().unwrap().english, "The Palace");
}

#[test]
fn recording_a_held_sign_again_keeps_it_and_names_the_job() {
    let scratch = Scratch::new("join");
    let library = library(&scratch);
    let hash = 0xaaaa_5555_aaaa_5555;
    library
        .record(library_sign("闘技場", hash, "Colosseum", "d11"))
        .unwrap();
    let again = library_sign("闘技場", hash ^ 0b10, "Arena", "d28");
    let recorded = library.record_all(vec![again.clone()]).unwrap();
    assert_eq!(
        recorded,
        Recorded {
            added: 0,
            joined: 1
        }
    );
    let held = library.lookup("闘技場", hash).unwrap().unwrap();
    assert_eq!(held.english, "Colosseum");
    assert_eq!(held.episodes, vec!["d11".to_string(), "d28".to_string()]);
    assert_eq!(library.record_all(vec![again]).unwrap().joined, 1);
    let held = library.lookup("闘技場", hash).unwrap().unwrap();
    assert_eq!(held.episodes.len(), 2);
    assert_eq!(library.size().unwrap().signs, 1);
}

#[test]
fn remove_takes_every_near_sign_of_the_text_and_clear_takes_all() {
    let scratch = Scratch::new("remove");
    let library = library(&scratch);
    let hash = 0x0f0f_0f0f_0f0f_0f0f;
    library
        .record_all(vec![
            library_sign("港", hash, "Harbor", "d11"),
            library_sign("港", !hash, "Port", "d12"),
            library_sign("海", hash, "Sea", "d11"),
        ])
        .unwrap();
    assert_eq!(library.remove("港", hash ^ 0b1).unwrap(), 1);
    assert_eq!(library.lookup("港", hash).unwrap(), None);
    assert!(library.lookup("港", !hash).unwrap().is_some());
    assert_eq!(library.size().unwrap().signs, 2);
    library.clear().unwrap();
    assert_eq!(library.size().unwrap().signs, 0);
    assert_eq!(library.lookup("海", hash).unwrap(), None);
}

#[test]
fn a_call_waits_while_another_handle_holds_the_file_and_then_succeeds() {
    let scratch = Scratch::new("busy");
    let library = library(&scratch);
    library
        .record(library_sign("海軍", 7, "Navy", "d11"))
        .unwrap();
    let held = Database::create(library.path()).unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        drop(held);
    });
    let started = Instant::now();
    let found = library.lookup("海軍", 7).unwrap();
    assert_eq!(found.unwrap().english, "Navy");
    assert!(started.elapsed() >= Duration::from_millis(250));
    release.join().unwrap();
}

#[test]
fn a_call_gives_up_when_the_file_stays_held() {
    let scratch = Scratch::new("held");
    let library = library(&scratch).waiting(Duration::from_millis(200));
    library
        .record(library_sign("海軍", 7, "Navy", "d11"))
        .unwrap();
    let held = Database::create(library.path()).unwrap();
    let error = library.lookup("海軍", 7).unwrap_err();
    assert!(error.to_string().contains("still held it"), "{error}");
    drop(held);
    assert!(library.lookup("海軍", 7).unwrap().is_some());
}

#[test]
fn a_library_of_another_layout_loses_its_signs() {
    let scratch = Scratch::new("layout");
    let library = library(&scratch);
    library
        .record(library_sign("海軍", 7, "Navy", "d11"))
        .unwrap();
    {
        let database = Database::create(library.path()).unwrap();
        let transaction = database.begin_write().unwrap();
        transaction
            .open_table(META)
            .unwrap()
            .insert(LAYOUT_KEY, LAYOUT_VERSION + 1)
            .unwrap();
        transaction.commit().unwrap();
    }
    assert_eq!(library.lookup("海軍", 7).unwrap(), None);
    assert_eq!(library.size().unwrap().signs, 0);
}

#[test]
fn only_the_translation_and_the_composition_read_the_library() {
    let reading: Vec<StepName> = StepName::ALL
        .into_iter()
        .filter(|step| reads_library(*step))
        .collect();
    assert_eq!(
        reading,
        vec![StepName::TextTranslate, StepName::TextCompose]
    );
}
