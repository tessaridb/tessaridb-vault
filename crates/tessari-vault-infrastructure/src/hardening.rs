//! What the process does to itself before it holds a key: it may write no core file, and on Linux it is not
//! dumpable at all, so neither a crash nor a debugger of the same user takes the keys it derives out of memory.

/// Why the process could not close itself to dumps.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum HardeningError {
    /// The core size limit could not be set to zero.
    #[error("the core size limit could not be set to zero (errno {0})")]
    CoreLimit(i32),
    /// The process could not be marked undumpable.
    #[error("the process could not be marked undumpable (errno {0})")]
    Dumpable(i32),
}

/// Set the core size limit to zero, hard and soft, and on Linux mark the process undumpable. Neither can be undone
/// by this process afterwards.
///
/// # Errors
///
/// [`HardeningError`] naming the call the system refused.
pub fn forbid_core_dumps() -> Result<(), HardeningError> {
    let none = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `setrlimit` reads one `rlimit` through the pointer, which names `none` for the call's duration.
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &raw const none) } != 0 {
        return Err(HardeningError::CoreLimit(errno()));
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: PR_SET_DUMPABLE takes its value as the second argument and no pointer.
        if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
            return Err(HardeningError::Dumpable(errno()));
        }
    }
    Ok(())
}

/// The error number the last refused call left.
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

#[cfg(test)]
#[path = "hardening_tests.rs"]
mod tests;
