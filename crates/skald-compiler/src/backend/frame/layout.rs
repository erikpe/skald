use super::model::{Base, FrameError, FramePolicy, Region};

pub(super) fn align(value: usize, alignment: usize) -> Result<usize, FrameError> {
    if !alignment.is_power_of_two() {
        return Err(FrameError::UnsupportedAlignment(alignment));
    }
    value
        .checked_add(alignment - 1)
        .map(|n| n & !(alignment - 1))
        .ok_or(FrameError::Overflow)
}
pub(super) fn local(
    cursor: &mut usize,
    bytes: usize,
    alignment: usize,
    policy: FramePolicy,
) -> Result<Region, FrameError> {
    if !alignment.is_power_of_two() || alignment > policy.alignment {
        return Err(FrameError::UnsupportedAlignment(alignment));
    }
    *cursor = align(
        cursor.checked_add(bytes).ok_or(FrameError::Overflow)?,
        alignment,
    )?;
    if *cursor > policy.max_frame {
        return Err(FrameError::UnsupportedSize(*cursor));
    }
    let offset = i64::try_from(*cursor)
        .map_err(|_| FrameError::Overflow)?
        .checked_neg()
        .ok_or(FrameError::Overflow)?;
    Ok(Region {
        base: Base::Frame,
        offset,
        bytes,
        alignment,
    })
}
pub(super) fn interval(region: Region) -> Result<(i64, i64), FrameError> {
    let last = i64::try_from(region.bytes.saturating_sub(1)).map_err(|_| FrameError::Overflow)?;
    Ok((
        region.offset,
        region
            .offset
            .checked_add(last)
            .ok_or(FrameError::Overflow)?,
    ))
}
pub(super) fn direct(region: Region, policy: FramePolicy) -> Result<bool, FrameError> {
    let (first, last) = interval(region)?;
    Ok(first >= policy.direct_min && last <= policy.direct_max)
}
