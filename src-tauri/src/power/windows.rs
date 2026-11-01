//! Windows: not yet supported. SetThreadExecutionState needs a system API call (unsafe),
//! which belongs in a small helper; done together with the Windows audio work (VRT-11).

use std::process::Child;

pub fn spawn(_why: &str) -> Result<Option<Child>, String> {
    Err("not supported on Windows yet (VRT-11)".into())
}
