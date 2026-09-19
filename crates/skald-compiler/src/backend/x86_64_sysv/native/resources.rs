use crate::backend::selected::{BankKind, ResourceCatalog, ResourceError, UnitId, ViewId};

/// Only low-byte views exist: AH/BH/CH/DH are deliberately unrepresentable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(in crate::backend) enum Gpr {
    Rax,
    Rcx,
    Rdx,
    Rbx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}
impl Gpr {
    pub(in crate::backend) const ALL: [Self; 16] = [
        Self::Rax,
        Self::Rcx,
        Self::Rdx,
        Self::Rbx,
        Self::Rsp,
        Self::Rbp,
        Self::Rsi,
        Self::Rdi,
        Self::R8,
        Self::R9,
        Self::R10,
        Self::R11,
        Self::R12,
        Self::R13,
        Self::R14,
        Self::R15,
    ];
    pub(in crate::backend) const ARGUMENTS: [Self; 6] = [
        Self::Rdi,
        Self::Rsi,
        Self::Rdx,
        Self::Rcx,
        Self::R8,
        Self::R9,
    ];
    pub(in crate::backend) fn reserved(self) -> bool {
        matches!(self, Self::Rsp | Self::Rbp)
    }
    pub(in crate::backend) fn preserved(self) -> bool {
        matches!(
            self,
            Self::Rbx | Self::Rbp | Self::Rsp | Self::R12 | Self::R13 | Self::R14 | Self::R15
        )
    }
}

/// Construction is checked and mutation stays private. Whole-register units
/// conservatively kill every overlapping view, including partial byte writes.
#[derive(Clone)]
pub(in crate::backend) struct NativeResources {
    catalog: ResourceCatalog,
    gprs: Vec<[ViewId; 4]>,
    xmm: Vec<[ViewId; 2]>,
    integer_choices: [Vec<ViewId>; 4],
    float_choices: Vec<ViewId>,
    divisor_choices: Vec<ViewId>,
    flags: UnitId,
    caller_clobbers: Vec<UnitId>,
    #[cfg_attr(not(test), allow(dead_code))]
    preserved: Vec<UnitId>,
}
impl NativeResources {
    pub(in crate::backend) fn for_profile(
        profile: crate::backend::plan::TargetProfile,
    ) -> Result<Self, &'static str> {
        use crate::backend::plan::{Abi, Architecture, Endianness};
        if profile.architecture != Architecture::X86_64
            || profile.abi != Abi::SysV
            || profile.data_layout.pointer_alignment != 8
            || profile.data_layout.pointer_bytes != 8
            || profile.data_layout.endianness != Endianness::Little
        {
            return Err("unsupported native target profile");
        }
        Self::new().map_err(|_| "invalid native resources")
    }
    pub(in crate::backend) fn new() -> Result<Self, ResourceError> {
        let mut catalog = ResourceCatalog::default();
        let integer = catalog.bank(BankKind::Integer);
        let float = catalog.bank(BankKind::Float);
        let mut caller_clobbers = vec![];
        let mut preserved = vec![];
        let mut gprs = vec![];
        for register in Gpr::ALL {
            let unit = catalog.unit()?;
            if register.preserved() {
                preserved.push(unit);
            } else {
                caller_clobbers.push(unit);
            }
            let mut views = vec![];
            for bits in [8, 16, 32, 64] {
                views.push(catalog.view(integer, bits, &[unit], register.reserved())?);
            }
            gprs.push(views.try_into().map_err(|_| ResourceError::Footprint)?);
        }
        let mut xmm = vec![];
        for _ in 0..16 {
            let unit = catalog.unit()?;
            caller_clobbers.push(unit);
            xmm.push([
                catalog.view(float, 64, &[unit], false)?,
                catalog.view(float, 128, &[unit], false)?,
            ]);
        }
        // Flags are a clobber-only unit, never a value bank or virtual operand.
        let flags = catalog.unit()?;
        caller_clobbers.push(flags);
        let integer_choices = std::array::from_fn(|width| {
            gprs.iter()
                .enumerate()
                .filter(|(index, _)| !Gpr::ALL[*index].reserved())
                .map(|(_, views): (_, &[ViewId; 4])| views[width])
                .collect()
        });
        let divisor_choices = gprs
            .iter()
            .enumerate()
            .filter(|(i, _)| !matches!(Gpr::ALL[*i], Gpr::Rax | Gpr::Rdx | Gpr::Rsp | Gpr::Rbp))
            .map(|(_, v)| v[3])
            .collect();
        let float_choices = xmm.iter().map(|views| views[0]).collect();
        Ok(Self {
            integer_choices,
            float_choices,
            divisor_choices,
            catalog,
            gprs,
            xmm,
            flags,
            caller_clobbers,
            preserved,
        })
    }
    pub(in crate::backend) fn divisor_views(&self) -> &[ViewId] {
        &self.divisor_choices
    }
    pub(in crate::backend) fn allocatable(&self, bank: BankKind, bits: u16) -> &[ViewId] {
        match (bank, bits) {
            (BankKind::Float, 64) => &self.float_choices,
            (BankKind::Integer, 8) => &self.integer_choices[0],
            (BankKind::Integer, 16) => &self.integer_choices[1],
            (BankKind::Integer, 32) => &self.integer_choices[2],
            (BankKind::Integer, 64) => &self.integer_choices[3],
            _ => &[],
        }
    }
    pub(in crate::backend) fn catalog(&self) -> &ResourceCatalog {
        &self.catalog
    }
    pub(in crate::backend) fn gpr(
        &self,
        register: Gpr,
        bits: u16,
    ) -> Result<ViewId, ResourceError> {
        let index = match bits {
            8 => 0,
            16 => 1,
            32 => 2,
            64 => 3,
            _ => return Err(ResourceError::Width),
        };
        Ok(self.gprs[register as usize][index])
    }
    pub(in crate::backend) fn xmm(
        &self,
        register: usize,
        bits: u16,
    ) -> Result<ViewId, ResourceError> {
        let index = match bits {
            64 => 0,
            128 => 1,
            _ => return Err(ResourceError::Width),
        };
        Ok(self.xmm.get(register).ok_or(ResourceError::Unknown)?[index])
    }
    pub(in crate::backend) fn flags(&self) -> UnitId {
        self.flags
    }
    pub(in crate::backend) fn caller_clobbers(&self) -> &[UnitId] {
        &self.caller_clobbers
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::backend) fn preserved_units(&self) -> &[UnitId] {
        &self.preserved
    }
}
