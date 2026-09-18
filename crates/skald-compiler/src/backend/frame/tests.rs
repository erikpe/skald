use super::{layout, model::*};
fn policy() -> FramePolicy {
    FramePolicy {
        alignment: 16,
        entry_remainder: 8,
        header_bytes: 8,
        incoming_base: 16,
        return_address: ReturnAddress::Stack {
            offset: 8,
            bytes: 8,
        },
        max_frame: i32::MAX as usize,
        direct_min: i32::MIN as i64,
        direct_max: i32::MAX as i64,
        materialization: None,
        address_scratch_group: 0,
    }
}
#[test]
fn local_regions_are_aligned_disjoint_and_checked_at_limits() {
    let mut cursor = 0;
    let a = layout::local(&mut cursor, 3, 4, policy()).unwrap();
    let b = layout::local(&mut cursor, 8, 8, policy()).unwrap();
    assert_eq!((a.offset, b.offset, cursor), (-4, -16, 16));
    assert!(b.offset + b.bytes as i64 <= a.offset);
    assert!(layout::direct(a, policy()).unwrap());
    assert_eq!(
        layout::local(&mut cursor, 1, 32, policy()),
        Err(FrameError::UnsupportedAlignment(32))
    );
    assert_eq!(
        layout::local(&mut cursor, usize::MAX, 1, policy()),
        Err(FrameError::Overflow)
    );
    cursor = 0;
    assert_eq!(
        layout::local(&mut cursor, i32::MAX as usize + 1, 1, policy()),
        Err(FrameError::UnsupportedSize(i32::MAX as usize + 1))
    );
    assert_eq!(layout::align(usize::MAX, 16), Err(FrameError::Overflow));
    assert_eq!(
        layout::align(i32::MAX as usize, 16).unwrap(),
        i32::MAX as usize + 1
    );
}
#[test]
fn displacement_checks_include_the_entire_access_extent() {
    let mut p = policy();
    p.direct_min = -32;
    p.direct_max = 31;
    let region = Region {
        base: Base::Frame,
        offset: -32,
        bytes: 64,
        alignment: 16,
    };
    assert!(layout::direct(region, p).unwrap());
    assert!(!layout::direct(
        Region {
            bytes: 65,
            ..region
        },
        p
    )
    .unwrap());
    assert!(!layout::direct(
        Region {
            offset: -33,
            ..region
        },
        p
    )
    .unwrap());
    assert_eq!(
        layout::interval(Region {
            offset: i64::MAX,
            bytes: 2,
            ..region
        }),
        Err(FrameError::Overflow)
    );
}
