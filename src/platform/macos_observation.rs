//! Unclassified, best-effort terminal foreground-group observations.
//! No session/descendant inference, provider knowledge, or generation guarantee.

use super::{macos_procargs, ForegroundJob, ForegroundProcess};

pub(crate) const ARGUMENT_POLICY: &str = "darwin_procargs2_lp64_v1";
const PROCESS_LIMIT: usize = 256;
const TEXT_LIMIT: usize = 64 * 1024;
// A native cwd is at most 1024 raw bytes, with up to 3x lossy expansion.
const CWD_RESERVE: usize = 3072;

#[derive(Clone, Debug, PartialEq, Eq)]
struct BsdInfo {
    pid: u32,
    pgid: u32,
    foreground_pgid: u32,
    start: (u64, u64),
    lp64: bool,
    comm: [u8; 16],
}

trait Source {
    fn bsd(&mut self, pid: u32) -> Option<BsdInfo>;
    /// Returned byte count, not PID count; a full buffer is inconclusive.
    fn group_pids(&mut self, pgid: u32, slots: &mut [i32]) -> Option<usize>;
    fn args_size(&mut self, pid: u32) -> Option<usize>;
    fn args_read(&mut self, pid: u32, bytes: &mut [u8]) -> Option<usize>;
}

fn checked_pid(pid: u32) -> Option<i32> {
    i32::try_from(pid).ok().filter(|&pid| pid > 0)
}

fn group_pids(source: &mut impl Source, pgid: u32) -> Option<Vec<u32>> {
    checked_pid(pgid)?;
    let mut capacity = 16usize;
    for _ in 0..6 {
        let mut slots = Vec::new();
        slots.try_reserve_exact(capacity).ok()?;
        slots.resize(capacity, 0i32);
        let bytes = source.group_pids(pgid, &mut slots)?;
        let available = capacity.checked_mul(std::mem::size_of::<i32>())?;
        if bytes > available || bytes % std::mem::size_of::<i32>() != 0 {
            return None;
        }
        if bytes == available {
            capacity = capacity.checked_mul(2)?.min(PROCESS_LIMIT + 1);
            continue;
        }
        slots.truncate(bytes / std::mem::size_of::<i32>());
        slots.retain(|&pid| pid > 0);
        slots.sort_unstable();
        slots.dedup();
        if slots.len() > PROCESS_LIMIT {
            return None;
        }
        let mut pids = Vec::new();
        pids.try_reserve_exact(slots.len()).ok()?;
        pids.extend(slots.into_iter().map(|pid| pid as u32));
        return Some(pids);
    }
    None
}

fn arguments(source: &mut impl Source, pid: u32, lp64: bool) -> Option<Vec<String>> {
    if !lp64 {
        return None;
    }
    let mut minimum = 0usize;
    for _ in 0..6 {
        let queried = source.args_size(pid)?;
        if queried == 0 || queried > macos_procargs::RAW_LIMIT {
            return None;
        }
        let capacity = queried.max(minimum);
        if capacity > macos_procargs::RAW_LIMIT {
            return None;
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).ok()?;
        bytes.resize(capacity, 0);
        let returned = source.args_read(pid, &mut bytes)?;
        if returned == 0 || returned > capacity {
            return None;
        }
        // XNU can return success with an undersized suffix. Do not decode a
        // successful capacity-filling read, even if it looks structurally valid.
        if returned == capacity {
            minimum = capacity.checked_mul(2)?.min(macos_procargs::RAW_LIMIT);
            if minimum <= capacity {
                return None;
            }
            continue;
        }
        bytes.truncate(returned);
        return macos_procargs::decode(&bytes, lp64);
    }
    None
}

fn argument_fields(argv: Vec<String>, name_len: usize) -> Option<(Vec<String>, String, String)> {
    let first = argv.first()?;
    let total = argv
        .iter()
        .try_fold(0usize, |n, arg| n.checked_add(arg.len()))?;
    let joined = total.checked_add(argv.len().checked_sub(1)?)?;
    let budget = total
        .checked_add(first.len())?
        .checked_add(joined)?
        .checked_add(name_len)?
        .checked_add(CWD_RESERVE)?;
    if budget > TEXT_LIMIT {
        return None;
    }
    let mut argv0 = String::new();
    argv0.try_reserve_exact(first.len()).ok()?;
    argv0.push_str(first);
    let mut cmdline = String::new();
    cmdline.try_reserve_exact(joined).ok()?;
    for (i, arg) in argv.iter().enumerate() {
        if i != 0 {
            cmdline.push(' ');
        }
        cmdline.push_str(arg);
    }
    Some((argv, argv0, cmdline))
}

fn collect(source: &mut impl Source, child_pid: u32) -> Option<ForegroundJob> {
    checked_pid(child_pid)?;
    let child = source.bsd(child_pid)?;
    if child.pid != child_pid {
        return None;
    }
    let pgid = child.foreground_pgid;
    let pids = group_pids(source, pgid)?;
    let mut processes = Vec::new();
    processes.try_reserve_exact(pids.len()).ok()?;
    for pid in pids {
        let Some(before) = source.bsd(pid) else {
            continue;
        };
        if before.pid != pid || before.pgid != pgid {
            continue;
        }
        let end = before.comm.iter().position(|&b| b == 0).unwrap_or(16);
        let Some(name) = std::str::from_utf8(&before.comm[..end])
            .ok()
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let args =
            arguments(source, pid, before.lp64).and_then(|argv| argument_fields(argv, name.len()));
        // Reject visible PID reuse, membership changes, and width changes. Even
        // equality cannot detect every exec or concurrent runtime memory change.
        let Some(after) = source.bsd(pid) else {
            continue;
        };
        if before.pid != after.pid
            || before.start != after.start
            || before.pgid != after.pgid
            || before.lp64 != after.lp64
            || before.comm != after.comm
        {
            continue;
        }
        let mut owned_name = String::new();
        owned_name.try_reserve_exact(name.len()).ok()?;
        owned_name.push_str(name);
        let (argv, argv0, cmdline) = match args {
            Some((argv, argv0, cmdline)) => (Some(argv), Some(argv0), Some(cmdline)),
            None => (None, None, None),
        };
        processes.push(ForegroundProcess {
            pid,
            name: owned_name,
            argv,
            argv0,
            cmdline,
        });
    }
    if processes.is_empty() {
        return None;
    }
    Some(ForegroundJob {
        process_group_id: pgid,
        processes,
    })
}

#[cfg(target_os = "macos")]
pub(super) fn foreground_job(child_pid: u32) -> Option<ForegroundJob> {
    collect(&mut native::Native, child_pid)
}

#[cfg(target_os = "macos")]
mod native {
    use super::*;

    pub(super) struct Native;
    // Public SDK sys/proc_info.h, XNU d4514f0... (xnu-11215.81.4).
    // libc 0.2.183 exposes neither PROC_FLAG_LP64 nor PROC_PGRP_ONLY.
    const PROC_FLAG_LP64: u32 = 0x10;
    const PROC_PGRP_ONLY: u32 = 2;

    impl Source for Native {
        fn bsd(&mut self, pid: u32) -> Option<BsdInfo> {
            let pid = checked_pid(pid)?;
            let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
            let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
            // SAFETY: initialized, correctly sized writable libc structure.
            let returned = unsafe {
                libc::proc_pidinfo(
                    pid,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    info.as_mut_ptr().cast(),
                    size,
                )
            };
            if returned != size {
                return None;
            }
            // SAFETY: proc_pidinfo reported the entire structure initialized.
            let info = unsafe { info.assume_init() };
            let mut comm = [0; 16];
            for (out, raw) in comm.iter_mut().zip(info.pbi_comm) {
                *out = raw as u8;
            }
            Some(BsdInfo {
                pid: info.pbi_pid,
                pgid: info.pbi_pgid,
                foreground_pgid: info.e_tpgid,
                start: (info.pbi_start_tvsec, info.pbi_start_tvusec),
                lp64: info.pbi_flags & PROC_FLAG_LP64 != 0,
                comm,
            })
        }

        fn group_pids(&mut self, pgid: u32, slots: &mut [i32]) -> Option<usize> {
            checked_pid(pgid)?;
            let size = i32::try_from(
                slots
                    .len()
                    .checked_mul(std::mem::size_of::<libc::pid_t>())?,
            )
            .ok()?;
            // SAFETY: slots are aligned pid_t storage and size matches the slice.
            let returned = unsafe {
                libc::proc_listpids(PROC_PGRP_ONLY, pgid, slots.as_mut_ptr().cast(), size)
            };
            usize::try_from(returned).ok().filter(|&n| n > 0)
        }

        fn args_size(&mut self, pid: u32) -> Option<usize> {
            sysctl(pid, None)
        }

        fn args_read(&mut self, pid: u32, bytes: &mut [u8]) -> Option<usize> {
            sysctl(pid, Some(bytes))
        }
    }

    fn sysctl(pid: u32, bytes: Option<&mut [u8]>) -> Option<usize> {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, checked_pid(pid)?];
        let (pointer, mut size) = match bytes {
            Some(bytes) => (bytes.as_mut_ptr().cast::<libc::c_void>(), bytes.len()),
            None => (std::ptr::null_mut(), 0),
        };
        // SAFETY: read-only MIB, optional writable slice with its exact length;
        // no new value is provided. Both pointers outlive the call.
        let result = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                3,
                pointer,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        (result == 0).then_some(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Fake {
        info: BTreeMap<u32, BsdInfo>,
        pids: Vec<i32>,
        raw: Option<Vec<u8>>,
        size: Option<usize>,
        returned: Option<usize>,
        group_returned: Option<usize>,
        reads: usize,
        queries: usize,
        capacities: Vec<usize>,
        args_capacities: Vec<usize>,
        change: Option<BsdInfo>,
    }

    fn info(pid: u32) -> BsdInfo {
        BsdInfo {
            pid,
            pgid: 7,
            foreground_pgid: 7,
            start: (10, 20),
            lp64: true,
            comm: *b"raw-name\0\0\0\0\0\0\0\0",
        }
    }

    fn fake() -> Fake {
        Fake {
            info: [(1, info(1)), (2, info(2)), (3, info(3))].into(),
            pids: vec![3, 0, -1, 2, 3],
            raw: Some(macos_procargs::fixture(
                8,
                &[b"", b"-full/path", b"", b"a b"],
            )),
            size: None,
            returned: None,
            group_returned: None,
            reads: 0,
            queries: 0,
            capacities: Vec::new(),
            args_capacities: Vec::new(),
            change: None,
        }
    }

    impl Source for Fake {
        fn bsd(&mut self, pid: u32) -> Option<BsdInfo> {
            if self.reads > 0 && self.change.as_ref().is_some_and(|i| i.pid == pid) {
                return self.change.clone();
            }
            self.info.get(&pid).cloned()
        }
        fn group_pids(&mut self, pgid: u32, slots: &mut [i32]) -> Option<usize> {
            assert_eq!(pgid, 7);
            self.capacities.push(slots.len());
            let count = slots.len().min(self.pids.len());
            slots[..count].copy_from_slice(&self.pids[..count]);
            Some(self.group_returned.unwrap_or(count * 4))
        }
        fn args_size(&mut self, _pid: u32) -> Option<usize> {
            self.queries += 1;
            self.size.or_else(|| self.raw.as_ref().map(Vec::len))
        }
        fn args_read(&mut self, _pid: u32, bytes: &mut [u8]) -> Option<usize> {
            self.reads += 1;
            self.args_capacities.push(bytes.len());
            let raw = self.raw.as_ref()?;
            let count = bytes.len().min(raw.len());
            bytes[..count].copy_from_slice(&raw[..count]);
            Some(self.returned.unwrap_or(count))
        }
    }

    #[test]
    fn neutral_sorted_deduped_raw_fields_share_one_decode() {
        assert_eq!(ARGUMENT_POLICY, "darwin_procargs2_lp64_v1");
        let mut source = fake();
        let job = collect(&mut source, 1).unwrap();
        assert_eq!(job.process_group_id, 7);
        assert_eq!(
            job.processes.iter().map(|p| p.pid).collect::<Vec<_>>(),
            [2, 3]
        );
        for process in job.processes {
            assert_eq!(process.name, "raw-name");
            assert_eq!(process.argv0.as_deref(), Some(""));
            assert_eq!(process.argv.unwrap(), ["", "-full/path", "", "a b"]);
            assert_eq!(process.cmdline.as_deref(), Some(" -full/path  a b"));
        }
        // Exact-sized first reads must be retried with headroom.
        assert_eq!(source.reads, 4);
    }

    #[test]
    fn no_argv0_normalization() {
        for raw in ["-login", "/absolute/name", "relative/name"] {
            let mut source = fake();
            source.raw = Some(macos_procargs::fixture(8, &[raw.as_bytes()]));
            let process = collect(&mut source, 1).unwrap().processes.remove(0);
            assert_eq!(process.argv0.as_deref(), Some(raw));
        }
    }

    #[test]
    fn unavailable_arguments_keep_names_and_omit_all_argument_fields() {
        for mode in 0..8 {
            let mut source = fake();
            match mode {
                0 => source.raw = None,
                1 => source.raw = Some(vec![0; 10]),
                2 => source.size = Some(macos_procargs::RAW_LIMIT + 1),
                3 => source.returned = Some(usize::MAX),
                4 => source.returned = Some(0),
                5 => source.size = Some(0),
                6 => {
                    for info in source.info.values_mut() {
                        info.lp64 = false;
                    }
                }
                _ => source.raw = Some(macos_procargs::fixture(8, &vec![&[b'x'; 4096][..]; 16])),
            }
            let job = collect(&mut source, 1).unwrap();
            for process in job.processes {
                assert_eq!(process.name, "raw-name");
                assert!(
                    process.argv.is_none() && process.argv0.is_none() && process.cmdline.is_none()
                );
            }
            if mode == 2 || mode == 5 || mode == 6 {
                assert_eq!(source.reads, 0);
            }
        }
    }

    #[test]
    fn pid_boundary_membership_names_and_visible_changes() {
        for pid in [0, i32::MAX as u32 + 1, u32::MAX] {
            let mut source = fake();
            assert!(collect(&mut source, pid).is_none());
            assert!(source.capacities.is_empty());
        }
        assert_eq!(checked_pid(i32::MAX as u32), Some(i32::MAX));
        for mode in 0..8 {
            let mut source = fake();
            source.pids = vec![2];
            match mode {
                0 => source.info.get_mut(&1).unwrap().foreground_pgid = u32::MAX,
                1 => source.info.get_mut(&2).unwrap().pgid = 8,
                2 => source.info.get_mut(&2).unwrap().comm = [0xff; 16],
                3 => source.info.get_mut(&2).unwrap().comm = [0; 16],
                4 => {
                    let mut changed = info(2);
                    changed.start.0 += 1;
                    source.change = Some(changed);
                }
                5 => {
                    let mut changed = info(2);
                    changed.pgid = 8;
                    source.change = Some(changed);
                }
                6 => {
                    let mut changed = info(2);
                    changed.lp64 = false;
                    source.change = Some(changed);
                }
                _ => source.info.get_mut(&1).unwrap().pid = 99,
            }
            assert!(collect(&mut source, 1).is_none(), "mode {mode}");
        }
        let mut source = fake();
        source.info.get_mut(&2).unwrap().comm = *b"1234567890123456";
        assert_eq!(
            collect(&mut source, 1).unwrap().processes[0].name,
            "1234567890123456"
        );
    }

    #[test]
    fn bounded_group_query_allows_256_but_never_reports_overflow_as_absence() {
        let mut source = fake();
        source.pids = (1..=256).rev().collect();
        let pids = group_pids(&mut source, 7).unwrap();
        assert_eq!(pids.len(), 256);
        assert_eq!(source.capacities, [16, 32, 64, 128, 256, 257]);
        assert_eq!(pids[0], 1);
        for count in [257, 258, 1024] {
            let mut source = fake();
            source.pids = (1..=count).collect();
            assert!(group_pids(&mut source, 7).is_none());
            assert_eq!(source.capacities.len(), 6);
            assert_eq!(source.capacities.last(), Some(&257));
        }
        for bytes in [1, 3, 65, usize::MAX] {
            let mut source = fake();
            source.group_returned = Some(bytes);
            assert!(group_pids(&mut source, 7).is_none());
        }
    }

    #[test]
    fn sysctl_full_read_above_half_cap_recovers_with_ceiling_headroom() {
        let mut source = fake();
        source.size = Some(614_400);
        let mut raw = macos_procargs::fixture(8, &[b"/raw/wrapper", b"", b"operand"]);
        raw.resize(614_400, b'x');
        source.raw = Some(raw);

        assert_eq!(
            arguments(&mut source, 2, true).unwrap(),
            ["/raw/wrapper", "", "operand"]
        );
        assert_eq!(source.args_capacities, [614_400, macos_procargs::RAW_LIMIT]);
        assert_eq!(source.queries, 2);
    }

    #[test]
    fn sysctl_full_read_at_ceiling_refuses_even_decodable_bytes() {
        for initial in [614_400, macos_procargs::RAW_LIMIT] {
            let mut source = fake();
            source.size = Some(initial);
            let mut raw = macos_procargs::fixture(8, &[b"/raw/wrapper", b"operand"]);
            raw.resize(macos_procargs::RAW_LIMIT, b'x');
            assert!(macos_procargs::decode(&raw, true).is_some());
            source.raw = Some(raw);

            assert!(arguments(&mut source, 2, true).is_none());
            let expected = if initial < macos_procargs::RAW_LIMIT {
                vec![initial, macos_procargs::RAW_LIMIT]
            } else {
                vec![macos_procargs::RAW_LIMIT]
            };
            assert_eq!(source.args_capacities, expected);
            assert_eq!(source.queries, expected.len());
        }
    }

    #[test]
    fn sysctl_retry_exhaustion_and_cap_are_unavailable() {
        let mut source = fake();
        source.size = Some(8);
        source.raw = Some(vec![0; 8192]);
        assert!(arguments(&mut source, 2, true).is_none());
        assert_eq!(source.reads, 6);
        let mut source = fake();
        source.size = Some(macos_procargs::RAW_LIMIT);
        source.raw = Some(vec![0; macos_procargs::RAW_LIMIT]);
        assert!(arguments(&mut source, 2, true).is_none());
        assert_eq!(source.reads, 1);
    }

    #[test]
    fn exact_combined_text_budget_before_join() {
        // 2 * total argv + argv0 + separators + name + cwd reserve.
        let name = 16;
        let argv = vec!["".into(), "x".repeat(31_223)];
        let fields = argument_fields(argv, name).unwrap();
        assert_eq!(
            fields.0.iter().map(String::len).sum::<usize>()
                + fields.1.len()
                + fields.2.len()
                + name
                + CWD_RESERVE,
            TEXT_LIMIT - 1
        );
        assert!(argument_fields(vec!["".into(), "x".repeat(31_224)], name).is_none());
        assert!(argument_fields(Vec::new(), 0).is_none());
    }

    #[test]
    fn decoded_arguments_at_exact_export_budget_and_one_byte_over() {
        let mut argv = vec!["ab".to_owned()];
        argv.extend(std::iter::repeat_n("x".repeat(4096), 7));
        argv.push("x".repeat(2545));
        for overflow in [false, true] {
            let mut args = argv.clone();
            if overflow {
                args.last_mut().unwrap().push('x');
            }
            let refs: Vec<_> = args.iter().map(|s| s.as_bytes()).collect();
            let mut source = fake();
            source.pids = vec![2];
            source.info.get_mut(&2).unwrap().comm = *b"1234567890123456";
            source.raw = Some(macos_procargs::fixture(8, &refs));
            let process = collect(&mut source, 1).unwrap().processes.remove(0);
            assert_eq!(process.argv.is_none(), overflow);
            assert_eq!(process.argv0.is_none(), overflow);
            assert_eq!(process.cmdline.is_none(), overflow);
            if !overflow {
                assert_eq!(
                    process.argv.unwrap().iter().map(String::len).sum::<usize>()
                        + process.argv0.unwrap().len()
                        + process.cmdline.unwrap().len()
                        + process.name.len()
                        + CWD_RESERVE,
                    TEXT_LIMIT
                );
            }
        }
    }
}
