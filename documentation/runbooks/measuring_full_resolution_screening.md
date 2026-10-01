**Status:** live

# Measuring full-resolution screening on the host

The host steps that fix the measured constants of full-resolution screening, the detector engine,
the localized video's segment encode and the 6.5 GB VRAM cap, then compare fresh runs of
Dressrosa 11 and 28 with the [M6 baseline](/documentation/research/m6_baseline.md), check the
localized videos in VLC and mpv, ship the AppImage and merge the branch to `main`. Run it once,
in order, on the owner's machine with nothing else using the GPU; it takes most of a day, mostly
the two benches and four whole jobs.

## Prerequisites

- The branch `claude/yuv-decoding-detector-vv3grn` checked out, with every commit of the work
  merged into it and the four checks passing (`cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `cargo gates`).
- The app's three release binaries built as in step 12 of the
  [development environment](/documentation/runbooks/development_environment.md#steps) runbook,
  and the models and GPU runtime downloaded as in its step 8; the TensorRT runtime folder in the
  app's runtime folder, as the [AppImage runbook](/documentation/runbooks/building_the_appimage.md)
  describes it.
- Dressrosa 11 and 28 in `/run/media/system/Main_storage/Media/one_pace/`, and the baseline's
  copies of their reports, step records, subtitle files and `text_verify` output in
  `one_pace/_m6_baseline/d11/` and `d28/`.
- The `claude` CLI signed in, for the whole jobs; the window closed during every measurement.

## Reference facts

| What | Where or which |
|---|---|
| Dressrosa 11 | `[Muhn Pace] Dressrosa 11.mp4`: 44,489 frames, H.264 1920×1080, 24 fps |
| Dressrosa 28 | `[Muhn Pace] Dressrosa 28.mp4` (named as Dressrosa 11 is): 38,177 frames |
| Measurement folders | `$HOME/tbd-fullres/`: `bench/`, `determinism/`, `fresh/` |
| Constants the benches fix | `ScreenShape::INITIAL` (screening batch and pool) and `CONFIRM_POOL_MIB` in `crates/inference/src/ocr/pool/mod.rs`; `TEXT_DETECT_VRAM_MIB` in `crates/pipeline/src/graph/gpu.rs`; the segment presets in `crates/media_io/src/encode/segments/args.rs` and the whole-video HEVC preset in `crates/media_io/src/encode/mod.rs` |
| Defaults the checks may flip | the CUDA search mode (`SearchMode`'s default, `Fast` or `Deterministic`) in `crates/inference/src/ocr/detector_pool/`; the detector engine (`DetectorEngine`'s default) in `crates/job_model/src/onscreen/settings.rs` |
| Limits | whole-job peak RAM within 24 GB; every GPU worker within 6.5 GB (6,656 MiB) of VRAM |

The storage layout of this work is new, so every existing job runs all its steps once more:
resuming the baseline jobs is not possible. The comparison runs are fresh, and since Claude's
answers differ between runs from scratch, the visual and localized steps are judged on their own
outputs. A `<video>.localized.mkv` is never overwritten by a job that did not write it, so before
each run in a new work root the episode's localized video is moved aside, as steps 6 and 14 do.

## Steps

1. Build the validation tool, in the container from the repository root.

   ```bash
   cargo build --release -p visual_validation
   ```

   **Expected:** `Finished release profile`; `target/release/visual_validation` exists.

2. Run the detect-bench on Dressrosa 11 on the host, TensorRT rows first, then the CUDA rows.
   Each TensorRT engine is built once and cached under `<app data>/tensorrt/`, so the first run
   reports its engine build time apart from screening. The bench's rows are: the pool × batch
   sweep of the screening session (batch 2, 4 and 8 against several pools), one session against
   two with GPU busy %, `Fast` against `Deterministic` boxes on the same frames, and TensorRT FP16
   and FP32 rows for the screening and confirmation sessions. The sweep's grid is
   `--sweep-batches` (default `2,4,8`) by `--sweep-pools-mib` (default `1536,2048,2560,3072`);
   `--shape-batch` and `--shape-pool-mib` fix the shape the later sections use (else the fastest
   sweep row); `--confirm-pools-mib` gives the confirmation rows' pools (default
   `1536,2048,2560,3072`); `--trt-cache` names the engine cache (default under the system's
   temporary folder, so a second run reuses it); `--vram-cap-mib` bounds TensorRT's workspace (default
   6656); `--frames-count` sets how many box images `--frames-dir` gets (default 6); and
   `--no-decode`, `--no-oar-ocr`, `--no-sweep`, `--no-sessions`, `--no-search`, `--no-tensorrt`
   and `--no-pool-confirm` skip sections.

   ```bash
   distrobox-host-exec target/release/visual_validation detect-bench "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" --frames-dir "$HOME/tbd-fullres/bench/d11-frames" --trt-cache "$HOME/tbd-fullres/bench/tensorrt"
   ```

   **Expected:** Markdown tables with, per row, frames per second, GPU busy %, peak VRAM and
   boxes found; the sweep rows also say whether the CUDA graph and NHWC were accepted and which
   search mode ran, and the TensorRT rows give the engine build time. The two-session row is
   busier on the GPU than the one-session row, which shows the two streams overlap.
   `d11-frames/` holds sample PNGs with the production pool's full-resolution CUDA boxes in blue
   (3 px), its TensorRT FP16 boxes in green (1 px) and the 640-wide proxy's boxes, scaled up, in
   magenta (1 px): look through them for writing only one of them finds, and judge whether FP16
   loses writing CUDA finds.

3. Run the same bench on Dressrosa 28.

   ```bash
   distrobox-host-exec target/release/visual_validation detect-bench "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 28.mp4" --frames-dir "$HOME/tbd-fullres/bench/d28-frames" --trt-cache "$HOME/tbd-fullres/bench/tensorrt"
   ```

   **Expected:** the same tables; the TensorRT rows reuse the cached engines, so their engine
   build time is near zero.

4. Fix the measured constants, in the container. From the sweep, take the fastest screening
   batch and pool whose worker peak, with both sessions, stays within 6.5 GB on both episodes;
   take each confirmation pool from the confirmation rows; set `text_detect`'s VRAM need to the
   measured peak of the chosen pair plus 256 MiB, as every other GPU step's need is its measured
   peak plus 256 MiB. Edit the constants named under Reference facts, then run the tests.

   ```bash
   cargo test --workspace
   ```

   **Expected:** every test passes. Rebuild the three release binaries as in step 12 of the
   development environment runbook.

5. Choose the TensorRT engine: open the window on the host, set Settings, On-screen Text,
   Detector engine to TensorRT, and close the window.

   ```bash
   distrobox-host-exec target/release/tbd-subtitles gui
   ```

   **Expected:** the choice is saved at once to `~/.config/tbd-subtitles/settings.toml`.

6. Move Dressrosa 11's localized video aside, so the fresh job may write its own.

   ```bash
   mkdir -p "$HOME/tbd-fullres/aside" && mv "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.localized.mkv" "$HOME/tbd-fullres/aside/d11-before-determinism.localized.mkv"
   ```

   **Expected:** no output. If there is no such file, `mv` says so and nothing needs moving.

7. Run Dressrosa 11 once in the determinism work root.

   ```bash
   distrobox-host-exec target/release/tbd-subtitles process "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" --work-root "$HOME/tbd-fullres/determinism" --no-library
   ```

   **Expected:** a `> step` and `✓ step` line for every step, then the subtitle file beside the
   video and the job's `report.md`. The output step moves the subtitle files already beside the
   video into the job's `backup/` folder.

8. Save the detection document.

   ```bash
   target/release/tbd-subtitles dump "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" outputs text_detect --work-root "$HOME/tbd-fullres/determinism" > "$HOME/tbd-fullres/determinism/tensorrt-1.json"
   ```

   **Expected:** exit 0 and one JSON object in the file. While a run of the job is still going it
   exits 1 with "process N is running this job; dump it after that run ends".

9. Run detection again on the same job, with the engine cached.

   ```bash
   distrobox-host-exec target/release/tbd-subtitles process "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" --work-root "$HOME/tbd-fullres/determinism" --no-library --rerun text_detect
   ```

   **Expected:** `> text_detect` and `✓ text_detect`; when the new document equals the first, the
   steps after it print `= step`, still valid.

10. Save the second document and compare the two.

    ```bash
    target/release/tbd-subtitles dump "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" outputs text_detect --work-root "$HOME/tbd-fullres/determinism" > "$HOME/tbd-fullres/determinism/tensorrt-2.json" && cmp "$HOME/tbd-fullres/determinism/tensorrt-1.json" "$HOME/tbd-fullres/determinism/tensorrt-2.json"
    ```

    **Expected:** no output from `cmp`: the cached TensorRT engine gives the same document.

11. Repeat with the CUDA engine: set Detector engine to CUDA as in step 5, then repeat steps 7
    to 10 in the same work root, naming the files `cuda-1.json` and `cuda-2.json`. The detection
    fingerprint covers the engine, so step 7 reruns `text_detect` on CUDA.

    ```bash
    cmp "$HOME/tbd-fullres/determinism/cuda-1.json" "$HOME/tbd-fullres/determinism/cuda-2.json"
    ```

    **Expected:** no output when the CUDA path repeats exactly. If they differ, switch the CUDA
    path's default search mode to `Deterministic`, rebuild, run the check again, and record which
    mode holds and why in a new entry of the
    [on-screen detection decisions](/documentation/decisions/onscreen_detection.md). If TensorRT
    is faster in the bench and gave the same document twice with its engine cached, flip the
    default detector engine to TensorRT; record that in a new entry of the
    [inference engine decisions](/documentation/decisions/inference_engines.md).

12. Measure the localized video's encoders on the host. The encode-bench times, on a clip, the
    x264 presets and the NVENC presets p1 to p7, for both the segment H.264 and the whole-video
    HEVC encode, over a clip that starts at `--start` (default 600 s) and runs `--duration`
    seconds (default 60).

    ```bash
    distrobox-host-exec target/release/visual_validation encode-bench "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4"
    ```

    **Expected:** one row per encoder and preset with frames per second, size and PSNR against the
    source. Before the first run the presets were x264 `-preset slow` and NVENC `p7 -tune hq`
    for segments and `p6` for the whole-video HEVC encode.

13. Set the presets the encode-bench supports, with the owner: `X264_SEGMENT_PRESET` and
    `NVENC_SEGMENT_PRESET` in `crates/media_io/src/encode/segments/args.rs`, and the whole-video
    HEVC preset in `crates/media_io/src/encode/mod.rs`,
    then run the tests and rebuild the three release binaries as in step 4.

    ```bash
    cargo test --workspace
    ```

    **Expected:** every test passes.

14. Move both episodes' localized videos aside before the fresh runs.

    ```bash
    mv "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.localized.mkv" "$HOME/tbd-fullres/aside/d11-determinism.localized.mkv" ; mv "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 28.localized.mkv" "$HOME/tbd-fullres/aside/d28-before.localized.mkv"
    ```

    **Expected:** no output, or `mv` saying a file is not there.

15. Run Dressrosa 11 from scratch with the defaults the steps above settled.

    ```bash
    distrobox-host-exec target/release/tbd-subtitles process "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" --work-root "$HOME/tbd-fullres/fresh" --no-library
    ```

    **Expected:** every step runs (`>` then `✓`), none is skipped; the subtitle files, the
    localized video and `report.md` are written.

16. Run Dressrosa 28 from scratch the same way.

    ```bash
    distrobox-host-exec target/release/tbd-subtitles process "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 28.mp4" --work-root "$HOME/tbd-fullres/fresh" --no-library
    ```

    **Expected:** as in step 15.

17. Save each job's step records beside its report, for the comparison and the record.

    ```bash
    target/release/tbd-subtitles dump "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" step_records --work-root "$HOME/tbd-fullres/fresh" > "$HOME/tbd-fullres/fresh/d11-step_records.jsonl"
    ```

    **Expected:** one JSON object per line, one per step. Do the same for Dressrosa 28. Then
    compare each `report.md` and its step records with the baseline's copies in
    `one_pace/_m6_baseline/`:
    - each step's wall time, and whether `text_detect` now ends before `adjudicate` does;
    - the occurrences found and the replacements `text_verify` approved;
    - the times of `text_mask`, `text_inpaint` and `text_verify`;
    - `localized_video`'s time and file size, its segments and frames re-encoded, its frames
      copied, and any reason for a whole-video encode;
    - whole-job peak RAM within 24 GB, and every worker's peak VRAM within 6.5 GB.

18. Decode each localized video whole on the host.

    ```bash
    distrobox-host-exec ffmpeg -v error -i "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.localized.mkv" -f null -
    ```

    **Expected:** no output: no decoding error anywhere, the joins included. Do the same for
    Dressrosa 28.

19. Count each localized video's frames.

    ```bash
    distrobox-host-exec ffprobe -v error -select_streams v:0 -count_packets -show_entries stream=nb_read_packets -of csv=p=0 "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.localized.mkv"
    ```

    **Expected:** `44489`, the source's count; `38177` for Dressrosa 28.

20. Play each localized video with its subtitle file in mpv, then in VLC, across every join. The
    joins sit at the keyframes around each replaced sign, so play every occurrence Check Text
    shows as "Replaced in the video" from a few seconds before it to a few seconds after, and
    seek across it once.

    ```bash
    distrobox-host-exec mpv --sub-file="/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.localized.ass" "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.localized.mkv"
    ```

    **Expected:** no stall, no corrupt frame and the audio in sync at every join, in mpv and in
    VLC (`distrobox-host-exec vlc` with the same video, which loads the `.localized.ass` beside
    it); the dialogue moves to the top over lettered English.

21. Build the AppImage, in the container.

    ```bash
    cargo appimage
    ```

    **Expected:** the build steps, then `dist/TBD-subtitles-<version>-<git short>-x86_64.AppImage`
    and its size; `dist/TBD-subtitles-x86_64.AppImage` is the stable name.

22. Smoke-test it on the host.

    ```bash
    distrobox-host-exec dist/TBD-subtitles-x86_64.AppImage --version
    ```

    **Expected:** `tbd-subtitles <version>`.

23. Re-import `dist/TBD-subtitles-x86_64.AppImage` into Gear Lever and launch the app once from
    the application menu, as in step 4 of the
    [AppImage runbook](/documentation/runbooks/building_the_appimage.md#steps).

    **Expected:** the window opens; Settings, On-screen Text shows the detector engine and NVDEC
    settings and the localized video's encoder.

24. Write the research record from the
    [research snapshot template](/documentation/standards/templates/research_snapshot.md), only
    now that the measurement exists: the bench tables, the constants and defaults chosen and
    why, the determinism results, the presets, and the comparison of step 17 with the baseline.
    Tick the roadmap items the owner keeps, in the same commit, then run the gates.

    ```bash
    cargo gates
    ```

    **Expected:** every gate ends with `OK — N check(s), all held` and the command exits 0.

25. Merge the branch to `main`, from the repository root on `main`; it is not left standing.

    ```bash
    git merge --ff-only claude/yuv-decoding-detector-vv3grn
    ```

    **Expected:** a fast-forward to the branch's last commit. If `main` has moved, rebase the
    branch onto it, run the four checks again and merge once more; then push `main` and delete
    the branch.

## Verify

```bash
git log --oneline -1 origin/main
```

**Expected:** the commit that adds the research record and ticks the roadmap items, on
`origin/main`, with the branch merged.

## Troubleshooting

- **"waiting for GPU memory: N MiB free, M needed":** another program holds GPU memory; the step
  waits up to ten minutes, then fails asking to close other GPU programs and retry. Close them (a
  browser with hardware acceleration, another run of the app) and press Try Again or rerun.
- **A sweep row prints an error:** that pool or batch does not fit; the bench goes on with the
  other rows. Leave the pair out of the choice in step 4.
- **The step's notes say a session reopened without the CUDA graph or NHWC:** the provider refused
  them, and the session runs without; record it in the research record, since it changes the
  speed the sweep measured.
- **The first TensorRT run is slow:** it builds its engines; the run reports `engine_build_s`
  apart. A driver or TensorRT update changes the engine's key, so the next run builds again.
- **The localized video's report line gives a reason for a whole-video encode:** the source is not
  constant-frame-rate H.264, or a join check failed; the reason names which. Record it; a failed
  check on these episodes is a bug to fix before acceptance.
- **`localized_video` fails asking to move a `.localized.mkv` away:** the file was written by
  another job; move it aside as in step 14 and run the job again.

## Related documentation

- [Roadmap](/documentation/roadmap.md#m6--24-gb-workstation-throughput) — the items this
  measures, in M6 and M8, and the M6.5 items it informs.
- [Memory profiles](/documentation/optimizations/memory_profiles.md) — full-resolution
  screening, the memory wait, decoding and the queues.
- [Visual and video](/documentation/optimizations/visual_and_video.md#3-re-encoding-only-what-changed)
  — the segment encode.
- [On-screen detection decisions](/documentation/decisions/onscreen_detection.md) — screening,
  confirmation and the search mode.
- [M6 baseline](/documentation/research/m6_baseline.md) — the numbers the fresh runs are
  compared with.
- [Development environment](/documentation/runbooks/development_environment.md) — building the
  release binaries and the host checks.
