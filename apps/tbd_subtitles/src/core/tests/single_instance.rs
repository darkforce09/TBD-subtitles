use super::*;

use std::sync::atomic::{AtomicUsize, Ordering};

/// An empty folder of its own under the temporary folder, short enough for a socket path.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-si-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn first(dir: &Path) -> Instance {
    match claim(dir).unwrap() {
        Claim::First(instance) => instance,
        Claim::Running => panic!("expected the first claim in {}", dir.display()),
    }
}

fn message(videos: &[&str]) -> HandOff {
    HandOff {
        videos: videos.iter().map(PathBuf::from).collect(),
        start: true,
        raise: false,
    }
}

/// A wake that counts its calls.
fn counting_wake() -> (Arc<OnceLock<Wake>>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let wake = Arc::new(OnceLock::new());
    let set = wake.set(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    }) as Wake);
    assert!(set.is_ok());
    (wake, calls)
}

/// `flock` belongs to the open file description, so a second open in this process conflicts.
#[test]
fn a_second_claim_finds_the_first_running_until_it_is_dropped() {
    let dir = scratch("claims");
    let instance = first(&dir);
    assert!(dir.join(SOCKET_FILE).exists());
    assert!(matches!(claim(&dir).unwrap(), Claim::Running));
    drop(instance);
    assert!(
        !dir.join(SOCKET_FILE).exists(),
        "dropping removes the socket"
    );
    let again = first(&dir);
    drop(again);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_claim_replaces_the_socket_a_dead_instance_left() {
    let dir = scratch("stale");
    drop(UnixListener::bind(dir.join(SOCKET_FILE)).unwrap());
    assert!(
        dir.join(SOCKET_FILE).exists(),
        "the stale socket file stays behind"
    );
    assert!(UnixStream::connect(dir.join(SOCKET_FILE)).is_err());
    let instance = first(&dir);
    assert!(UnixStream::connect(dir.join(SOCKET_FILE)).is_ok());
    drop(instance);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_hand_off_arrives_with_absolute_paths_and_wakes_the_window() {
    let dir = scratch("round");
    let (wake, calls) = counting_wake();
    let received = serve(first(&dir), wake);
    let sent = message(&["episode 11.mkv", "/videos/episode 12.mkv"]);
    hand_off(&dir, &sent, Duration::from_secs(2)).unwrap();
    let got = received.recv_timeout(Duration::from_secs(2)).unwrap();
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(
        got,
        HandOff {
            videos: vec![
                cwd.join("episode 11.mkv"),
                PathBuf::from("/videos/episode 12.mkv")
            ],
            start: true,
            raise: false,
        }
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(received);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_malformed_line_is_refused_and_delivers_nothing() {
    let dir = scratch("malformed");
    let (wake, calls) = counting_wake();
    let received = serve(first(&dir), wake);
    for line in [
        "not json\n",
        "{\"videos\":[],\"start\":true,\"raise\":false,\"extra\":1}\n",
    ] {
        let mut stream = UnixStream::connect(dir.join(SOCKET_FILE)).unwrap();
        stream.write_all(line.as_bytes()).unwrap();
        let mut reply = String::new();
        BufReader::new(&stream).read_line(&mut reply).unwrap();
        let reply: Reply = serde_json::from_str(reply.trim_end()).unwrap();
        assert!(!reply.ok, "{line}");
        assert!(reply.error.is_some_and(|error| !error.is_empty()));
    }
    assert!(received.recv_timeout(Duration::from_millis(200)).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(received);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_hand_off_times_out_when_the_lock_holder_never_listens() {
    let dir = scratch("silent");
    let lock = File::create(dir.join(LOCK_FILE)).unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(claim(&dir).unwrap(), Claim::Running));
    let started = Instant::now();
    let error = hand_off(&dir, &message(&["/a.mkv"]), Duration::from_millis(300)).unwrap_err();
    assert!(started.elapsed() >= Duration::from_millis(300));
    assert!(error.contains("no running window answered"), "{error}");
    drop(lock);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_dropped_receiver_refuses_the_next_hand_off_and_frees_the_lock() {
    let dir = scratch("closing");
    let received = serve(first(&dir), Arc::new(OnceLock::new()));
    drop(received);
    let error = hand_off(&dir, &message(&["/a.mkv"]), Duration::from_secs(2)).unwrap_err();
    assert_eq!(error, "the window is closing");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match claim(&dir).unwrap() {
            Claim::First(instance) => {
                drop(instance);
                break;
            }
            Claim::Running if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(20));
            }
            Claim::Running => panic!("the serving thread kept the lock"),
        }
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_runtime_folder_holds_a_private_instance_folder() {
    let runtime = scratch("runtime");
    let dir = instance_dir_under(Some(runtime.clone())).unwrap();
    assert_eq!(dir, runtime.join("tbd-subtitles"));
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(instance_dir().unwrap().is_dir());
    fs::remove_dir_all(runtime).unwrap();
}
