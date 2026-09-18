//! Two pointer words: previous TLS head and current source location. Every TLS
//! access and record mutation is an ordinary explicit native memory event.
use super::*;
use crate::backend::{graph::SelectedObjectId, lir::TraceAction, plan::ArtifactId};
impl<'p> Recipes<'_, 'p> {
    pub(in crate::backend::x86_64_sysv::native::selected) fn trace(
        &mut self,
        block: Block<'p>,
        action: &TraceAction<SelectedObjectId>,
        initial: Option<ArtifactId>,
    ) -> Result<()> {
        let record = match *action {
            TraceAction::PushFrame { record }
            | TraceAction::PopFrame { record }
            | TraceAction::ReplaceLocation { record, .. } => record,
        };
        let address = self.value(ScalarType::DataAddress)?;
        self.emit(
            block,
            Opcode::ObjectAddress {
                object: record,
                out: address,
            },
        )?;
        match *action {
            TraceAction::PushFrame { .. } => {
                let tls = self.tls(block)?;
                let previous = self.trace_load(block, tls, None)?;
                self.emit(
                    block,
                    Opcode::TraceStore {
                        address,
                        value: previous,
                        record: Some(record),
                    },
                )?;
                self.location(
                    block,
                    address,
                    record,
                    initial.ok_or(SelectedBuildError::TypeMismatch)?,
                )?;
                self.emit(
                    block,
                    Opcode::TraceStore {
                        address: tls,
                        value: address,
                        record: None,
                    },
                )?;
            }
            TraceAction::ReplaceLocation { location, .. } => {
                self.location(block, address, record, location)?;
            }
            TraceAction::PopFrame { .. } => {
                let previous = self.trace_load(block, address, Some(record))?;
                let tls = self.tls(block)?;
                self.emit(
                    block,
                    Opcode::TraceStore {
                        address: tls,
                        value: previous,
                        record: None,
                    },
                )?;
            }
        }
        Ok(())
    }
    fn tls(&mut self, block: Block<'p>) -> Result<ValueRef> {
        let out = self.value(ScalarType::DataAddress)?;
        self.emit(block, Opcode::TlsAddress { out })?;
        Ok(out)
    }
    fn trace_load(
        &mut self,
        block: Block<'p>,
        address: ValueRef,
        record: Option<SelectedObjectId>,
    ) -> Result<ValueRef> {
        let out = self.value(ScalarType::DataAddress)?;
        self.emit(
            block,
            Opcode::TraceLoad {
                address,
                out,
                record,
            },
        )?;
        Ok(out)
    }
    fn location(
        &mut self,
        block: Block<'p>,
        base: ValueRef,
        record: SelectedObjectId,
        location: ArtifactId,
    ) -> Result<()> {
        let offset = self.constant(block, Constant::U64(8))?;
        let address = self.value(ScalarType::DataAddress)?;
        self.emit(
            block,
            Opcode::ByteOffset {
                base,
                offset,
                out: address,
            },
        )?;
        let value = self.value(ScalarType::DataAddress)?;
        self.emit(
            block,
            Opcode::SymbolAddress {
                symbol: location,
                out: value,
            },
        )?;
        self.emit(
            block,
            Opcode::TraceStore {
                address,
                value,
                record: Some(record),
            },
        )
    }
}
