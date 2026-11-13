//! Reviewed LP64 KERN_PROCARGS2 layout, not authenticated execution arguments.
//!
//! Source: apple-oss-distributions/xnu tag xnu-11215.81.4, commit
//! d4514f0bc1d3f944c22d92e68b646ac3fb40d452, bsd/kern/kern_exec.c
//! (exec_save_path, exec_extract_strings) and bsd/kern/kern_sysctl.c
//! (sysctl_procargsx). Exec aligns the original `executable_path=` + path
//! region to the TARGET pointer width. Sysctl removes the 16-byte key without
//! repacking and prepends argc. Thus reply argv starts at 4 + round_up(L, 8),
//! NOT round_up(4 + L, 8), where L includes the path's NUL. Never skip NULs
//! beyond that calculated padding: direct exec permits empty argv[0].
//! Runtime stack mutation and stale exec data remain possible. This policy
//! limits interpretation under that reviewed layout; it does not attest origin.

pub(super) const RAW_LIMIT: usize = 1024 * 1024;
const ARG_LIMIT: usize = 4096;
const TEXT_LIMIT: usize = 64 * 1024;

/// LP64 must be established from the target's BSD flags, not observer width.
pub(super) fn decode(buf: &[u8], target_lp64: bool) -> Option<Vec<String>> {
    if !target_lp64 || buf.len() > RAW_LIMIT {
        return None;
    }
    let argc = i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?);
    if !(1..=128).contains(&argc) {
        return None;
    }
    let rest = buf.get(4..)?;
    let path_len = rest.iter().position(|&b| b == 0)?.checked_add(1)?;
    let start = path_len.checked_add(7)? & !7;
    if rest.get(path_len..start)?.iter().any(|&b| b != 0) {
        return None;
    }
    let mut cursor = start;
    let mut total = 0usize;
    let mut argv = Vec::new();
    argv.try_reserve_exact(argc as usize).ok()?;
    for _ in 0..argc {
        let remaining = rest.get(cursor..)?;
        // No unbounded scan or allocation for an individual argument.
        let end = remaining.iter().take(ARG_LIMIT + 1).position(|&b| b == 0)?;
        let arg = lossy(&remaining[..end], ARG_LIMIT)?;
        total = total.checked_add(arg.len())?;
        if total > TEXT_LIMIT {
            return None;
        }
        argv.push(arg);
        cursor = cursor.checked_add(end)?.checked_add(1)?;
    }
    // Deliberately do not inspect any bytes after exactly argc terminated strings.
    Some(argv)
}

/// Fallible, bounded equivalent of String::from_utf8_lossy, including grouping
/// of incomplete/invalid UTF-8 sequences into replacement characters.
fn lossy(mut bytes: &[u8], limit: usize) -> Option<String> {
    let mut out = String::new();
    out.try_reserve_exact(bytes.len().checked_mul(3)?.min(limit))
        .ok()?;
    loop {
        match std::str::from_utf8(bytes) {
            Ok(text) => {
                if out.len().checked_add(text.len())? > limit {
                    return None;
                }
                out.push_str(text);
                return Some(out);
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if out.len().checked_add(valid)?.checked_add(3)? > limit {
                    return None;
                }
                out.push_str(std::str::from_utf8(&bytes[..valid]).ok()?);
                out.push('\u{fffd}');
                bytes = &bytes[valid + error.error_len().unwrap_or(bytes.len() - valid)..];
            }
        }
    }
}

#[cfg(test)]
pub(super) fn fixture(path_len: usize, argv: &[&[u8]]) -> Vec<u8> {
    assert!(path_len > 0);
    // Model exec's ORIGINAL region first, then sysctl's prefix removal and
    // argc insertion, independently of the decoder's reply-offset calculation.
    let mut original = b"executable_path=".to_vec();
    original.extend(std::iter::repeat_n(b'p', path_len - 1));
    original.push(0);
    original.resize(original.len().div_ceil(8) * 8, 0);
    for arg in argv {
        original.extend_from_slice(arg);
        original.push(0);
    }
    let mut buf = (argv.len() as i32).to_ne_bytes().to_vec();
    buf.extend_from_slice(&original[b"executable_path=".len()..]);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_path_residues_empty_first_and_later_operands_exact_argc() {
        for len in 1..=24 {
            let mut buf = fixture(len, &[b"", b"-raw/path", b"", b"tail", b""]);
            buf.extend_from_slice(b"SECRET=value\0invalid\xff");
            assert_eq!(
                decode(&buf, true).unwrap(),
                ["", "-raw/path", "", "tail", ""]
            );
        }
    }

    #[test]
    fn no_padding_is_required_when_original_path_region_aligned() {
        let buf = fixture(8, &[b"x"]);
        assert_eq!(buf.len(), 14);
        assert_eq!(decode(&buf, true).unwrap(), ["x"]);
    }

    #[test]
    fn padding_must_exist_and_be_zero() {
        for len in 1..8 {
            let mut buf = fixture(len, &[b"x"]);
            buf[4 + len] = 1;
            assert!(decode(&buf, true).is_none());
            assert!(decode(&buf[..4 + len], true).is_none());
        }
    }

    #[test]
    fn truncation_and_invalid_headers_fail_closed() {
        let buf = fixture(5, &[b"first", b"last"]);
        for end in 0..buf.len() {
            assert!(decode(&buf[..end], true).is_none());
        }
        for argc in [-1i32, 0, 129, i32::MAX] {
            let mut bad = buf.clone();
            bad[..4].copy_from_slice(&argc.to_ne_bytes());
            assert!(decode(&bad, true).is_none());
        }
        assert!(decode(&buf, false).is_none());
        assert!(decode(&vec![0; RAW_LIMIT + 1], true).is_none());
        assert!(decode(&[1, 0, 0, 0, b'x'], true).is_none());
    }

    #[test]
    fn decoded_and_aggregate_limits_include_lossy_expansion() {
        assert!(decode(&fixture(8, &[&vec![b'a'; 4096]]), true).is_some());
        assert!(decode(&fixture(8, &[&vec![b'a'; 4097]]), true).is_none());
        assert!(decode(&fixture(8, &[&vec![0xff; 1366]]), true).is_none());
        let arg = vec![b'a'; 4096];
        assert!(decode(&fixture(8, &vec![arg.as_slice(); 16]), true).is_some());
        assert!(decode(&fixture(8, &vec![arg.as_slice(); 17]), true).is_none());
        for bytes in [&b"a\xff\xf0\x90"[..], &b"\xe2\x82\xac"[..]] {
            assert_eq!(
                decode(&fixture(8, &[bytes]), true).unwrap()[0],
                String::from_utf8_lossy(bytes)
            );
        }
    }
}
