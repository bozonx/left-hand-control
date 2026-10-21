//! Complete snapshots of potentially held native keys, published before injection.

use std::collections::BTreeSet;
use std::io::{self, Read, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key {
    pub vk: u16,
    pub scan: u16,
    pub flags: u32,
}

pub(super) fn finish(
    held: &mut BTreeSet<Key>,
    events: impl IntoIterator<Item = (Key, bool)>,
) -> bool {
    let previous = held.clone();
    for (key, down) in events {
        if down {
            held.insert(key);
        } else {
            held.remove(&key);
        }
    }
    *held != previous
}

pub(super) fn write_snapshot(writer: &mut impl Write, held: &BTreeSet<Key>) -> io::Result<()> {
    let count = u16::try_from(held.len()).map_err(|_| io::Error::other("too many held keys"))?;
    let mut frame = Vec::with_capacity(2 + held.len() * 8);
    frame.extend(count.to_le_bytes());
    for key in held {
        frame.extend(key.vk.to_le_bytes());
        frame.extend(key.scan.to_le_bytes());
        frame.extend(key.flags.to_le_bytes());
    }
    writer.write_all(&frame)?;
    writer.flush()
}

/// Keep the last complete snapshot if the owner dies midway through a write.
pub(super) fn read_snapshots(reader: &mut impl Read, held: &mut BTreeSet<Key>) -> io::Result<()> {
    loop {
        let mut count = [0; 2];
        match reader.read_exact(&mut count) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        }
        let count = usize::from(u16::from_le_bytes(count));
        if count > 1024 {
            return Err(io::Error::other("invalid input guard snapshot"));
        }
        let mut next = BTreeSet::new();
        for _ in 0..count {
            let mut bytes = [0; 8];
            match reader.read_exact(&mut bytes) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
                Err(error) => return Err(error),
            }
            next.insert(Key {
                vk: u16::from_le_bytes([bytes[0], bytes[1]]),
                scan: u16::from_le_bytes([bytes[2], bytes[3]]),
                flags: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            });
        }
        *held = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Key = Key {
        vk: 0,
        scan: 0x1d,
        flags: 8,
    };
    const RIGHT_CTRL: Key = Key {
        vk: 0,
        scan: 0x1d,
        flags: 9,
    };

    #[test]
    fn shortcut_release_and_restore_keeps_the_modifier_held() {
        let mut held = BTreeSet::from([CTRL, RIGHT_CTRL]);
        assert!(finish(
            &mut held,
            [(CTRL, false), (RIGHT_CTRL, false), (CTRL, true)]
        ));
        assert_eq!(held, BTreeSet::from([CTRL]));
        assert!(!finish(&mut held, [(CTRL, true)]));
    }

    #[test]
    fn interrupted_publication_preserves_the_last_complete_hold_set() {
        let mut bytes = Vec::new();
        write_snapshot(&mut bytes, &BTreeSet::from([CTRL])).unwrap();
        let boundary = bytes.len();
        write_snapshot(&mut bytes, &BTreeSet::from([CTRL, RIGHT_CTRL])).unwrap();
        for end in boundary..bytes.len() {
            let mut held = BTreeSet::new();
            read_snapshots(&mut &bytes[..end], &mut held).unwrap();
            assert_eq!(held, BTreeSet::from([CTRL]), "interrupted at {end}");
        }
        let mut held = BTreeSet::new();
        read_snapshots(&mut bytes.as_slice(), &mut held).unwrap();
        assert_eq!(held, BTreeSet::from([CTRL, RIGHT_CTRL]));
    }

    #[test]
    fn completed_release_removes_keys_but_an_interrupted_release_does_not() {
        let mut bytes = Vec::new();
        write_snapshot(&mut bytes, &BTreeSet::from([CTRL])).unwrap();
        write_snapshot(&mut bytes, &BTreeSet::new()).unwrap();
        let mut held = BTreeSet::new();
        read_snapshots(&mut &bytes[..bytes.len() - 1], &mut held).unwrap();
        assert_eq!(held, BTreeSet::from([CTRL]));
        read_snapshots(&mut bytes.as_slice(), &mut held).unwrap();
        assert!(held.is_empty());
    }
}
