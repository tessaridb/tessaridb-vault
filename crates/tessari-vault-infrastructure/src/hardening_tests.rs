use super::forbid_core_dumps;

#[test]
fn after_it_no_core_file_can_be_written_and_on_linux_nothing_can_dump_the_process() {
    assert_eq!(forbid_core_dumps(), Ok(()));
    let mut held = libc::rlimit {
        rlim_cur: 1,
        rlim_max: 1,
    };
    // SAFETY: `getrlimit` writes one `rlimit` through the pointer, which names `held` for the call's duration.
    let read = unsafe { libc::getrlimit(libc::RLIMIT_CORE, &raw mut held) };
    assert_eq!(read, 0);
    assert_eq!((held.rlim_cur, held.rlim_max), (0, 0));
    #[cfg(target_os = "linux")]
    {
        // SAFETY: PR_GET_DUMPABLE takes no pointer and answers the flag as the return value.
        let dumpable = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) };
        assert_eq!(dumpable, 0);
    }
}
