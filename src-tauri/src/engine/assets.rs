//! What the engine downloads at first run, pinned by digest (decision-1, doc-2 §1.3).
//! Changing a pin is a deliberate update: check the ABI and re-run the VRT-29 comparisons.

/// libparakeet from LocalAI's backend image on quay.io (OCI registry API, anonymous).
pub struct EngineImage {
    pub registry: &'static str,
    pub repository: &'static str,
    /// Digest of the single-platform image manifest (the bytes are hashed on download).
    pub manifest: &'static str,
    /// Files to keep from the image layer: (path in the layer, file name in the engine folder).
    pub files: &'static [(&'static str, &'static str)],
    /// The library the helper loads.
    pub library: &'static str,
}

/// One file with a fixed size and sha256.
pub struct PinnedFile {
    pub url: &'static str,
    pub file_name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

const REGISTRY: &str = "https://quay.io";
const REPOSITORY: &str = "go-skynet/local-ai-backends";

/// latest-cpu-parakeet-cpp, linux/amd64, as of 2026-10-30. glibc, libstdc++ and libgcc
/// come from the OS; the image's own copies must not be used (wrong glibc).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const ENGINE: Option<EngineImage> = Some(EngineImage {
    registry: REGISTRY,
    repository: REPOSITORY,
    manifest: "sha256:8497e3e34b78602e84f45807c860402bca6cdff9057b612c548fe7f11579dffc",
    files: &[
        ("lib/libparakeet.so", "libparakeet.so"),
        ("lib/libgomp.so.1", "libgomp.so.1"),
    ],
    library: "libparakeet.so",
});

/// latest-metal-darwin-arm64-parakeet-cpp as of 2026-10-30 (not yet verified on a Mac).
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const ENGINE: Option<EngineImage> = Some(EngineImage {
    registry: REGISTRY,
    repository: REPOSITORY,
    manifest: "sha256:25bbd38213939328301a471cb1ae39536e5dd8738744097f697f42c024c63d38",
    files: &[("lib/libparakeet.dylib", "libparakeet.dylib")],
    library: "libparakeet.dylib",
});

/// No prebuilt libparakeet for this platform (Windows: built from source in VRT-23).
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64")
)))]
pub const ENGINE: Option<EngineImage> = None;

/// Hugging Face, pinned to revision 15a3c9ed of mudler/parakeet-cpp-gguf. Own mirror: VRT-33.
/// The one model for Dutch and English (VRT-32): ASR, VAD, diarization, speaker encoder.
pub const MODEL: PinnedFile = PinnedFile {
    url: "https://huggingface.co/mudler/parakeet-cpp-gguf/resolve/15a3c9ed8f067123fb96fd10ce06d1bd77d6c343/parakeet-bundle-standard.gguf",
    file_name: "parakeet-bundle-standard.gguf",
    size: 1_100_752_928,
    sha256: "85304e3bb49de04b84a7d0c76baaa7dedd1a0c02c0920078a99f92c00cffbdf6",
};

/// Licences and attributions of the bundle's models; shown in Settings and kept with it.
pub const NOTICE: PinnedFile = PinnedFile {
    url: "https://huggingface.co/mudler/parakeet-cpp-gguf/resolve/15a3c9ed8f067123fb96fd10ce06d1bd77d6c343/NOTICE-parakeet-bundle-standard.txt",
    file_name: "NOTICE-parakeet-bundle-standard.txt",
    size: 37_326,
    sha256: "73a1f19607e8cf3f74b742f7ae64d700dc535225c91c40616f4ce6be84aa848c",
};
