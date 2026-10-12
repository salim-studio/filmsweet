//! Equality and counting tests for the redundant-work removals: the old implementation is kept
//! here as the reference, the new one must give the same results.

use super::*;

/// The bitwise CRC-32 that `ts::crc32_mpeg` replaced.
fn crc32_bitwise(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= (b as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 { (crc << 1) ^ 0x04C1_1DB7 } else { crc << 1 };
        }
    }
    crc
}

/// Deterministic xorshift bytes.
fn noise(seed: u32, n: usize) -> Vec<u8> {
    let mut x = seed | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x >> 8) as u8
        })
        .collect()
}

#[test]
fn table_crc_equals_the_bitwise_crc() {
    // the standard check value of CRC-32/MPEG-2
    assert_eq!(ts::crc32_mpeg(b"123456789"), 0x0376_E6E7);
    assert_eq!(ts::crc32_mpeg(&[]), 0xFFFF_FFFF);
    for b in 0..=255u8 {
        assert_eq!(ts::crc32_mpeg(&[b]), crc32_bitwise(&[b]));
    }
    for n in [0usize, 1, 2, 3, 4, 5, 15, 16, 17, 183, 184, 1023, 1024, 4096] {
        for seed in 1..6u32 {
            let d = noise(seed * 7919 + n as u32, n);
            assert_eq!(ts::crc32_mpeg(&d), crc32_bitwise(&d), "len {n} seed {seed}");
        }
    }
    // a section followed by its own CRC checks to zero
    let mut sec = noise(5, 40);
    let crc = ts::crc32_mpeg(&sec);
    sec.extend(crc.to_be_bytes());
    assert_eq!(ts::crc32_mpeg(&sec), 0);
}
