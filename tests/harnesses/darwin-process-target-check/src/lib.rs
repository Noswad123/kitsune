//! Compile the actual isolated Darwin acquisition sources without the Kit app,
//! Carbon, Ghostty, or a fake target cfg. Entry points have no caller here.
#![allow(dead_code)]

#[path = "../../../../src/platform/foreground.rs"]
mod foreground;
#[cfg(any(target_os = "macos", test))]
pub(crate) use foreground::{ForegroundJob, ForegroundProcess};

#[cfg(any(target_os = "macos", test))]
#[path = "../../../../src/platform/macos_observation.rs"]
mod macos_observation;
#[cfg(any(target_os = "macos", test))]
#[path = "../../../../src/platform/macos_procargs.rs"]
mod macos_procargs;
