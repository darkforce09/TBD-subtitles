use super::*;

fn hash(size: u64, md5: &str) -> PacketHash {
    PacketHash {
        size,
        md5: md5.into(),
    }
}

#[test]
fn a_framemd5_list_reads_into_sizes_and_hashes_in_order() {
    let list = concat!(
        "#format: frame checksums\n",
        "#version: 2\n",
        "#tb 0: 1/1000\n",
        "#stream#, dts,        pts, duration,     size, hash\n",
        "0,        -19,         23,       41,     4950, 93bdaf5482248b4eee02416b674ca965\n",
        "0,         23,        148,       41,     1972, 628fd90cd97854021745c3d02fb72f5b\n",
        "0,         65,         65,       41,  garbage, 155a255518d4246d395f0228e664d606\n",
    );
    assert_eq!(
        parse_framemd5(list),
        [
            hash(4950, "93bdaf5482248b4eee02416b674ca965"),
            hash(1972, "628fd90cd97854021745c3d02fb72f5b"),
        ]
    );
}

#[test]
fn a_copied_packet_may_only_gain_parameter_sets() {
    let source = hash(4951, "raw");
    let bare = hash(4950, "bare");
    assert!(packet_kept(&source, &source, &bare, &bare));
    // A keyframe with the SPS and PPS before it.
    assert!(packet_kept(&source, &hash(4987, "headed"), &bare, &bare));
    // Changed bytes, or bytes lost, are not kept.
    assert!(!packet_kept(&source, &hash(4951, "other"), &bare, &bare));
    assert!(!packet_kept(
        &source,
        &hash(4987, "headed"),
        &bare,
        &hash(4950, "x")
    ));
    assert!(!packet_kept(&source, &hash(4900, "short"), &bare, &bare));
}

#[test]
fn only_copied_pieces_are_compared() {
    let source = Hashes {
        stored: vec![hash(10, "a"), hash(10, "b"), hash(10, "c")],
        without_headers: vec![hash(9, "a"), hash(9, "b"), hash(9, "c")],
    };
    let joined = Hashes {
        stored: vec![hash(10, "a"), hash(12, "new"), hash(10, "c")],
        without_headers: vec![hash(9, "a"), hash(11, "new"), hash(9, "c")],
    };
    let pieces = [
        Piece::Copy { first: 0, last: 0 },
        Piece::Encode { first: 1, last: 1 },
        Piece::Copy { first: 2, last: 2 },
    ];
    assert_eq!(compare_copied(&pieces, &source, &joined), Ok(()));
    let all_copied = [Piece::Copy { first: 0, last: 2 }];
    let reason = compare_copied(&all_copied, &source, &joined).unwrap_err();
    assert_eq!(
        reason.0,
        "packet 1 of the copied frames 0..=2 differs from the source's"
    );
    let past_end = [Piece::Copy { first: 0, last: 3 }];
    assert!(compare_copied(&past_end, &source, &joined).is_err());
}

#[test]
fn both_hash_lists_come_from_one_stream_copy() {
    let args = hash_args(
        Path::new("/v/out.mkv"),
        Path::new("/job/joined.framemd5"),
        Path::new("/job/joined.bare.framemd5"),
    );
    assert_eq!(
        args.join(" "),
        "-nostdin -hide_banner -v error -y -i /v/out.mkv -map 0:v:0 -c copy -f framemd5 \
         /job/joined.framemd5 -map 0:v:0 -c copy -bsf:v filter_units=remove_types=7|8 \
         -f framemd5 /job/joined.bare.framemd5"
    );
}
