//! Validate and freeze bounded access recipes using already checked scratch.
use super::{layout, model::*};
use crate::backend::{
    placement::{Assignment, Location, Site},
    selected::{Bundle, Payload},
};

impl<P: Payload> FramePlan<'_, '_, '_, P> {
    pub(super) fn check_location(&self, location: Location) -> Result<(), FrameError> {
        let Some(region) = self.location(location)? else {
            return Ok(());
        };
        if layout::direct(region, self.policy)? {
            Ok(())
        } else {
            Err(FrameError::UnsupportedDisplacement(region.offset))
        }
    }
    pub(in crate::backend) fn location(
        &self,
        location: Location,
    ) -> Result<Option<Region>, FrameError> {
        let region = match location {
            Location::Resource(_) => return Ok(None),
            Location::Storage(id) => *self
                .regions
                .get(&Key::Storage(id.index()))
                .ok_or(FrameError::UnknownLocation(location))?,
            Location::Abi {
                signature,
                area,
                index,
            } => {
                let mut region = *self
                    .regions
                    .get(&Key::Abi(signature, area))
                    .ok_or(FrameError::UnknownLocation(location))?;
                let slot = self
                    .placement
                    .selected()
                    .draft()
                    .context()
                    .abi_layout(signature, area)
                    .ok_or(FrameError::MissingAbiLayout(signature, area))?
                    .slots
                    .get(index)
                    .copied()
                    .ok_or(FrameError::UnknownLocation(location))?;
                region.offset = region
                    .offset
                    .checked_add(i64::try_from(slot.offset).map_err(|_| FrameError::Overflow)?)
                    .ok_or(FrameError::Overflow)?;
                region.bytes = slot.bytes;
                region.alignment = slot.alignment;
                region
            }
        };
        Ok(Some(region))
    }
    pub(super) fn objects(&mut self, site: Site, payload: &P) -> Result<(), FrameError> {
        let d = payload.describe();
        let mut materialized_steps = 0usize;
        for &object in d.objects.iter() {
            let region = self.regions[&Key::Object(object)];
            let recipe = if layout::direct(region, self.policy)? {
                AddressRecipe::Direct
            } else {
                let Some((lo, hi, steps)) = self.policy.materialization else {
                    return Err(FrameError::UnsupportedDisplacement(region.offset));
                };
                let (first, last) = layout::interval(region)?;
                if first < lo || last > hi {
                    return Err(FrameError::UnsupportedDisplacement(region.offset));
                }
                let Some(Bundle::Bounded {
                    steps: bound,
                    ref scratch,
                }) = d.bundle
                else {
                    return Err(FrameError::UndeclaredAddressScratch(site));
                };
                materialized_steps = materialized_steps
                    .checked_add(usize::from(steps))
                    .ok_or(FrameError::Overflow)?;
                if usize::from(bound.get()) < materialized_steps {
                    return Err(FrameError::UndeclaredAddressScratch(site));
                }
                let pointer_bits = self
                    .placement
                    .selected()
                    .draft()
                    .context()
                    .catalog()
                    .plan()
                    .profile()
                    .data_layout
                    .pointer_bytes
                    * 8;
                let mut address_scratch = None;
                for (group, requirement) in scratch.iter().enumerate() {
                    if group != self.policy.address_scratch_group
                        || requirement.representation.bits() as usize != pointer_bits
                        || requirement.representation.bank()
                            != crate::backend::selected::BankKind::Integer
                    {
                        continue;
                    }
                    if let Some(Location::Resource(view)) =
                        self.placement.assignment(Assignment::Scratch {
                            site,
                            group,
                            slot: 0,
                        })
                    {
                        address_scratch = Some(view);
                        break;
                    }
                }
                AddressRecipe::Materialized {
                    scratch: address_scratch.ok_or(FrameError::UndeclaredAddressScratch(site))?,
                    steps,
                }
            };
            self.object_accesses.insert((site, object), recipe);
        }
        Ok(())
    }
}
