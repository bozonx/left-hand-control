//! Exercise the real IPC server without compiling the UI library as a test target.

pub use slint_shell::command;

#[allow(dead_code)]
#[path = "../src/ipc.rs"]
mod ipc;
