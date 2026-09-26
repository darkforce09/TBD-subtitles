//! With the `crispasr` feature, points the binary's rpath at the libcrispasr and ggml libraries
//! crispasr-sys built, which dependent binaries otherwise cannot find at run time; without it,
//! does nothing. libggml-cuda carries its own runpath, so the CUDA 13 libraries it needs come
//! from the `LD_LIBRARY_PATH` the worker is started with.

fn main() {
    for key in ["DEP_CRISPASR_LIBDIR", "DEP_CRISPASR_GGMLDIR"] {
        println!("cargo:rerun-if-env-changed={key}");
        if let Ok(dir) = std::env::var(key) {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{dir}");
        }
    }
}
