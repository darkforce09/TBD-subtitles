use super::*;

fn pieces() -> Vec<Piece> {
    vec![
        Piece::Copy { first: 0, last: 23 },
        Piece::Encode {
            first: 24,
            last: 47,
        },
        Piece::Copy {
            first: 48,
            last: 143,
        },
    ]
}

fn request() -> JoinRequest {
    JoinRequest {
        folder: PathBuf::from("/job/pieces"),
        source: PathBuf::from("/videos/ep 11.mkv"),
        video_offset_s: 0.023,
        frame_rate: (24000, 1001),
        output: PathBuf::from("/videos/ep 11.localized.mkv.part"),
    }
}

#[test]
fn each_piece_has_its_own_file() {
    let folder = Path::new("/job/pieces");
    let [copy, encode, _] = pieces()[..] else {
        unreachable!()
    };
    assert_eq!(
        piece_file(folder, 0, copy),
        PathBuf::from("/job/pieces/copy00000.mkv")
    );
    assert_eq!(
        piece_file(folder, 1, encode),
        PathBuf::from("/job/pieces/encode00001.mkv")
    );
}

#[test]
fn the_list_starts_every_piece_at_its_first_frames_exact_time() {
    // 24 frames at 24000/1001 last 1.001 s; 96 frames 4.004 s.
    assert_eq!(
        join_list(&pieces(), (24000, 1001)),
        "ffconcat version 1.0\n\
         file 'copy00000.mkv'\nduration 1.001000\n\
         file 'encode00001.mkv'\nduration 1.001000\n\
         file 'copy00002.mkv'\nduration 4.004000\n"
    );
    // One frame at 24000/1001 lasts 41708.33 us: the rounding never accumulates.
    let single_frames: Vec<Piece> = (0..3)
        .map(|frame| Piece::Copy {
            first: frame,
            last: frame,
        })
        .collect();
    let list = join_list(&single_frames, (24000, 1001));
    let durations: Vec<&str> = list
        .lines()
        .filter_map(|line| line.strip_prefix("duration "))
        .collect();
    assert_eq!(durations, ["0.041708", "0.041709", "0.041708"]);
}

#[test]
fn the_join_offsets_the_video_and_copies_the_sources_audio_and_chapters() {
    let args = join_args(Path::new("/job/pieces/pieces.ffconcat"), &request());
    assert_eq!(
        args.join(" "),
        "-nostdin -hide_banner -v error -y -f concat -safe 0 -auto_convert 0 -itsoffset 0.023000 \
         -i /job/pieces/pieces.ffconcat -i /videos/ep 11.mkv -map 0:v:0 -map 1:a? \
         -map_chapters 1 -map_metadata 1 -sn -dn -c copy -max_muxing_queue_size 4096 \
         -f matroska /videos/ep 11.localized.mkv.part"
    );
}

#[test]
fn an_invalid_join_is_refused_before_anything_runs() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    let timeout = Duration::from_secs(10);
    let mut in_place = request();
    in_place.output = in_place.source.clone();
    let mut no_rate = request();
    no_rate.frame_rate = (0, 1);
    let mut offset = request();
    offset.video_offset_s = f64::NAN;
    for bad in [in_place, no_rate, offset] {
        assert!(matches!(
            join_pieces(&programs, &pieces(), &bad, timeout, None),
            Err(MediaError::Parse(_))
        ));
    }
    assert!(join_pieces(&programs, &[], &request(), timeout, None).is_err());
}

#[test]
fn nothing_is_cut_when_no_piece_is_copied() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    let only_encoded = [Piece::Encode { first: 0, last: 9 }];
    copy_pieces(
        &programs,
        Path::new("/videos/ep.mkv"),
        &only_encoded,
        Path::new("/job"),
        Duration::from_secs(10),
        None,
    )
    .unwrap();
}
