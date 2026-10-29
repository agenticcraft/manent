//! Transcription engine (doc-2 §1.3): the `Engine` trait hides LocalAI so a direct
//! parakeet.cpp FFI engine can replace it later. Implemented in VRT-14 and VRT-15.

mod localai;
