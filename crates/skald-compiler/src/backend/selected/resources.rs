//! Overlap is a property of units, never inferred from bank membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum BankKind {
    Integer,
    Float,
}
macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(not(test), allow(dead_code))]
        pub(in crate::backend) struct $name(usize);
    };
}
id!(BankId);
id!(UnitId);
id!(ViewId);
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ResourceError {
    Unknown,
    Empty,
    Duplicate,
    Width,
    Reserved,
    Footprint,
}
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug)]
pub(in crate::backend) struct ResourceView {
    pub bank: BankId,
    pub bits: u16,
    pub units: Vec<UnitId>,
    pub reserved: bool,
}
#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ResourceCatalog {
    banks: Vec<BankKind>,
    units: usize,
    views: Vec<ResourceView>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl ResourceCatalog {
    pub(in crate::backend) fn banks(
        &self,
    ) -> impl ExactSizeIterator<Item = (BankId, BankKind)> + '_ {
        self.banks
            .iter()
            .copied()
            .enumerate()
            .map(|(i, kind)| (BankId(i), kind))
    }
    pub(in crate::backend) fn views(
        &self,
    ) -> impl ExactSizeIterator<Item = (ViewId, &ResourceView)> {
        self.views
            .iter()
            .enumerate()
            .map(|(i, view)| (ViewId(i), view))
    }
    pub(in crate::backend) fn units(&self) -> usize {
        self.units
    }
    pub(in crate::backend) fn bank(&mut self, kind: BankKind) -> BankId {
        let id = BankId(self.banks.len());
        self.banks.push(kind);
        id
    }
    pub(in crate::backend) fn unit(&mut self) -> Result<UnitId, ResourceError> {
        let id = UnitId(self.units);
        self.units = self.units.checked_add(1).ok_or(ResourceError::Footprint)?;
        Ok(id)
    }
    pub(in crate::backend) fn view(
        &mut self,
        bank: BankId,
        bits: u16,
        units: &[UnitId],
        reserved: bool,
    ) -> Result<ViewId, ResourceError> {
        self.banks.get(bank.0).ok_or(ResourceError::Unknown)?;
        if bits == 0 {
            return Err(ResourceError::Width);
        }
        if units.is_empty() {
            return Err(ResourceError::Empty);
        }
        let mut footprint = units.to_vec();
        footprint.sort_unstable();
        footprint.dedup();
        if footprint.len() != units.len() {
            return Err(ResourceError::Duplicate);
        }
        if footprint.iter().any(|unit| unit.0 >= self.units) {
            return Err(ResourceError::Unknown);
        }
        let id = ViewId(self.views.len());
        self.views.push(ResourceView {
            bank,
            bits,
            units: footprint,
            reserved,
        });
        Ok(id)
    }
    fn get(&self, view: ViewId) -> Result<&ResourceView, ResourceError> {
        self.views.get(view.0).ok_or(ResourceError::Unknown)
    }
    pub(in crate::backend) fn view_units(&self, view: ViewId) -> Result<&[UnitId], ResourceError> {
        Ok(&self.get(view)?.units)
    }
    pub(in crate::backend) fn overlaps(&self, a: ViewId, b: ViewId) -> Result<bool, ResourceError> {
        let a = self.get(a)?;
        let b = self.get(b)?;
        Ok(a.units.iter().any(|u| b.units.contains(u)))
    }
    pub(in crate::backend) fn require_view(
        &self,
        view: ViewId,
        bits: u16,
        kind: BankKind,
        allocatable: bool,
    ) -> Result<(), ResourceError> {
        let v = self.get(view)?;
        if v.bits != bits || self.banks[v.bank.0] != kind {
            return Err(ResourceError::Width);
        }
        if allocatable
            && self
                .views
                .iter()
                .any(|other| other.reserved && other.units.iter().any(|u| v.units.contains(u)))
        {
            return Err(ResourceError::Reserved);
        }
        Ok(())
    }
    /// Partial preservation names units of this view, not a bank-wide promise.
    pub(in crate::backend) fn preserved(
        &self,
        view: ViewId,
        units: &[UnitId],
    ) -> Result<bool, ResourceError> {
        let v = self.get(view)?;
        if units.iter().any(|u| u.0 >= self.units) {
            return Err(ResourceError::Unknown);
        }
        Ok(v.units.iter().all(|u| units.contains(u)))
    }
    pub(in crate::backend) fn require_unit(&self, unit: UnitId) -> Result<(), ResourceError> {
        if unit.0 < self.units {
            Ok(())
        } else {
            Err(ResourceError::Unknown)
        }
    }
}
