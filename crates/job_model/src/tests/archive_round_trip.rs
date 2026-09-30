//! The rkyv round trip every contract type's archive test runs.
//!
//! **Role:** archive a value, read the archive in place from a misaligned slice, bring it back
//! both through [`rkyv::access`] with [`rkyv::deserialize`] and through [`rkyv::from_bytes`], and
//! check that each copy equals the original.
//!
//! **Position:** test-only; declared by `lib.rs` and called by the `tests/archive.rs` file of each
//! module.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** the archive is always read from an odd address, so a pass proves the
//! `unaligned` layout that lets the job database read a record at any offset of a redb value.

use std::fmt::Debug;

use rkyv::api::high::{HighDeserializer, HighSerializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error;
use rkyv::ser::allocator::ArenaHandle;
use rkyv::util::AlignedVec;
use rkyv::{Archive, Deserialize, Serialize};

/// `bytes` copied to a buffer so that the copy starts at an odd address.
pub(crate) fn misaligned(bytes: &[u8]) -> (Vec<u8>, usize) {
    let mut buffer = vec![0u8; bytes.len() + 1];
    let start = usize::from((buffer.as_ptr() as usize).is_multiple_of(2));
    buffer[start..start + bytes.len()].copy_from_slice(bytes);
    assert_eq!(
        (buffer[start..].as_ptr() as usize) % 2,
        1,
        "the copy is misaligned"
    );
    (buffer, start)
}

/// Archive `value`, read it back from a misaligned copy and assert it comes back unchanged.
pub(crate) fn round_trip<T>(value: &T)
where
    T: Archive
        + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, Error>>
        + PartialEq
        + Debug,
    T::Archived:
        for<'a> CheckBytes<HighValidator<'a, Error>> + Deserialize<T, HighDeserializer<Error>>,
{
    let bytes = rkyv::to_bytes::<Error>(value).expect("the value archives");
    let (buffer, start) = misaligned(&bytes);
    let slice = &buffer[start..start + bytes.len()];

    let archived = rkyv::access::<T::Archived, Error>(slice).expect("the archive validates");
    let deserialized = rkyv::deserialize::<T, Error>(archived).expect("the archive deserialises");
    assert_eq!(&deserialized, value);

    let from_bytes = rkyv::from_bytes::<T, Error>(slice).expect("from_bytes reads the archive");
    assert_eq!(&from_bytes, value);
}
