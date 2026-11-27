//! Performance cores for the timed work. On macOS the scheduler may run a thread that waits for
//! the network on an efficiency core, which makes every requote slower than the benchmark's
//! figures. Setting the thread's quality of service to user-interactive makes the scheduler much
//! more likely to keep it on a performance core, as the benchmark does for its own threads.
//! macOS does not pass the class on to threads started later, so the quoting thread and each
//! runtime worker call [`prefer_performance_cores`] themselves. Elsewhere it does nothing.
//!
//! This is the only module in the examples with `unsafe`: one call into the system library,
//! allowed on that one function only.

/// Asks macOS to run the calling thread at the user-interactive quality of service. Returns
/// whether the class was set.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub fn prefer_performance_cores() -> bool {
    // From <pthread/qos.h>: QOS_CLASS_USER_INTERACTIVE.
    const USER_INTERACTIVE: u32 = 0x21;
    unsafe extern "C" {
        fn pthread_set_qos_class_self_np(class: u32, relative_priority: i32) -> i32;
    }
    // SAFETY: the function takes two integers and no pointers, so it reads and writes no memory
    // of this program. `USER_INTERACTIVE` is a valid `qos_class_t` and 0 is a valid relative
    // priority (it must be between -15 and 0). It changes only the scheduling class of the
    // calling thread and reports failure through its return value, which is checked.
    unsafe { pthread_set_qos_class_self_np(USER_INTERACTIVE, 0) == 0 }
}

/// Does nothing outside macOS, and returns `false`.
#[cfg(not(target_os = "macos"))]
pub fn prefer_performance_cores() -> bool {
    false
}
