//! Tells the code which FFmpeg API to use, from the version `ffmpeg-sys-next` found.

fn main() {
    println!("cargo:rustc-check-cfg=cfg(ffmpeg_supported_config)");
    println!("cargo:rerun-if-env-changed=DEP_FFMPEG_FFMPEG_7_1");
    // FFmpeg 7.1 lists what an encoder takes through avcodec_get_supported_config, and
    // later releases drop the per-codec lists it replaces.
    if std::env::var("DEP_FFMPEG_FFMPEG_7_1").is_ok_and(|value| value == "true") {
        println!("cargo:rustc-cfg=ffmpeg_supported_config");
    }
}
