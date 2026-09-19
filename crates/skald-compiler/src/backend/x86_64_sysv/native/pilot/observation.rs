/// Private requested-only checkpoints for the native architectural pilot.
///
/// Each flag renders the immutable product owned by that phase while its body is
/// resident. The public driver does not consume this type; architecture adoption
/// owns that adapter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::backend) struct NativePilotInspection {
    pub lowered: bool,
    pub selected: bool,
    pub physical: bool,
    pub placement: bool,
    pub frame: bool,
}

impl NativePilotInspection {
    pub(super) fn physical_checkpoint(self) -> bool {
        self.physical || self.placement || self.frame
    }
}
