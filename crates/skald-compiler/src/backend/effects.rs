//! Mandatory observable effects shared without executable phase dependencies.

use crate::identity::StaticFieldId;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum MemoryRegion<O> {
    Object(O),
    Static(StaticFieldId),
    Unknown,
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Effect<O> {
    Read(MemoryRegion<O>),
    Write(MemoryRegion<O>),
    Call,
    Allocate,
    Free,
    Report,
    HardTrap,
    TraceState,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct Effects<O> {
    entries: Vec<Effect<O>>,
}
impl<O> Default for Effects<O> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<O: Copy + Ord> Effects<O> {
    pub(in crate::backend) fn try_map_objects<R: Copy + Ord, E>(
        &self,
        mut map: impl FnMut(O) -> Result<R, E>,
    ) -> Result<Effects<R>, E> {
        let mut entries = Vec::with_capacity(self.entries.len());
        for effect in &self.entries {
            let region = |region: MemoryRegion<O>,
                          map: &mut dyn FnMut(O) -> Result<R, E>|
             -> Result<MemoryRegion<R>, E> {
                Ok(match region {
                    MemoryRegion::Object(id) => MemoryRegion::Object(map(id)?),
                    MemoryRegion::Static(id) => MemoryRegion::Static(id),
                    MemoryRegion::Unknown => MemoryRegion::Unknown,
                })
            };
            entries.push(match *effect {
                Effect::Read(r) => Effect::Read(region(r, &mut map)?),
                Effect::Write(r) => Effect::Write(region(r, &mut map)?),
                Effect::Call => Effect::Call,
                Effect::Allocate => Effect::Allocate,
                Effect::Free => Effect::Free,
                Effect::Report => Effect::Report,
                Effect::HardTrap => Effect::HardTrap,
                Effect::TraceState => Effect::TraceState,
            });
        }
        Ok(Effects::new(entries))
    }
    pub(in crate::backend) fn new(entries: impl IntoIterator<Item = Effect<O>>) -> Self {
        let mut entries: Vec<_> = entries.into_iter().collect();
        entries.sort_unstable();
        entries.dedup();
        Self { entries }
    }
    pub(in crate::backend) fn iter(&self) -> impl ExactSizeIterator<Item = &Effect<O>> {
        self.entries.iter()
    }
    pub(in crate::backend) fn contains(&self, effect: Effect<O>) -> bool {
        self.entries.binary_search(&effect).is_ok()
    }
    pub(in crate::backend) fn covers(&self, required: &Self) -> bool {
        required.entries.iter().all(|effect| {
            self.entries.binary_search(effect).is_ok()
                || match effect {
                    Effect::Read(_) => self.contains(Effect::Read(MemoryRegion::Unknown)),
                    Effect::Write(_) => self.contains(Effect::Write(MemoryRegion::Unknown)),
                    _ => false,
                }
        })
    }
    pub(in crate::backend) fn conservative_call() -> Self {
        Self::new([
            Effect::Call,
            Effect::Read(MemoryRegion::Unknown),
            Effect::Write(MemoryRegion::Unknown),
            Effect::Allocate,
            Effect::Free,
            Effect::Report,
            Effect::HardTrap,
            Effect::TraceState,
        ])
    }
}
