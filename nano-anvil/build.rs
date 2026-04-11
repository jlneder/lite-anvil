use std::path::PathBuf;

fn main() {
    // Link against the no-GL SDL3 build bundled in lib/sdl3-nogl/.
    // This SDL3 was built with -DSDL_OPENGL=OFF -DSDL_OPENGLES=OFF
    // -DSDL_VULKAN=OFF -DSDL_GPU=OFF -DSDL_RENDER=OFF to avoid
    // loading GPU driver libraries (~70MB on NVIDIA).
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let nogl_lib = manifest_dir.parent().unwrap().join("lib").join("sdl3-nogl");

    if nogl_lib.exists() {
        // Tell the linker to find libSDL3 here instead of the system path.
        println!("cargo::rustc-link-search=native={}", nogl_lib.display());

        // Set RPATH so the binary finds the bundled lib at runtime.
        // $ORIGIN/../lib/sdl3-nogl works for installed layouts where the
        // binary is in bin/ and the lib is in lib/sdl3-nogl/.
        // $ORIGIN works if the .so is placed next to the binary.
        println!(
            "cargo::rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib/sdl3-nogl:$ORIGIN/lib/sdl3-nogl:{}",
            nogl_lib.display()
        );
    }
}
