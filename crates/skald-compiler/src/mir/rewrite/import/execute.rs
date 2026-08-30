//! Atomic allocation and cloning for a prepared import.

use crate::mir::{MirBasicBlock, MirPathCondition, MirStorage, MirValue};

use super::super::{
    edit::{BlockPlacement, LogicalRecordIndex, MirCallableEdit},
    error::MirRewriteError,
    map::{map_instruction, map_logical_expression, map_path_condition_metadata, map_terminator},
    MirLocalIdentitySite,
};
use super::{
    mapper::RehomeMapper,
    model::{MirImportMaps, MirImportRequest, MirImportResult, MirImportSource},
    prepare::PreparedRequest,
};

impl MirCallableEdit {
    /// Atomically imports one explicitly bounded source region.
    pub(crate) fn import_region(
        &mut self,
        source: &MirImportSource,
        request: MirImportRequest,
    ) -> Result<MirImportResult, MirRewriteError> {
        if source.callable == self.callable() {
            return Err(MirRewriteError::ImportSourceMatchesDestination {
                callable: source.callable,
            });
        }
        let mut candidate = self.clone();
        let result = Importer::new(&mut candidate, source, request)?.run()?;
        *self = candidate;
        Ok(result)
    }
}

struct Importer<'edit, 'source> {
    destination: &'edit mut MirCallableEdit,
    source: &'source MirImportSource,
    request: PreparedRequest,
    maps: MirImportMaps,
}

impl<'edit, 'source> Importer<'edit, 'source> {
    fn new(
        destination: &'edit mut MirCallableEdit,
        source: &'source MirImportSource,
        request: MirImportRequest,
    ) -> Result<Self, MirRewriteError> {
        let request = PreparedRequest::new(source, destination, request)?;
        let maps = request.substitution_maps(source.callable, destination.callable());
        Ok(Self {
            destination,
            source,
            request,
            maps,
        })
    }

    fn run(mut self) -> Result<MirImportResult, MirRewriteError> {
        self.allocate_storage()?;
        self.allocate_values()?;
        self.allocate_blocks()?;
        self.allocate_guards();
        self.allocate_path_conditions()?;
        self.clone_blocks()?;
        self.clone_path_conditions()?;
        let logical_records = self.clone_logical_records()?;
        Ok(MirImportResult {
            maps: self.maps,
            logical_records,
        })
    }

    fn allocate_storage(&mut self) -> Result<(), MirRewriteError> {
        for (source_id, kind) in self.request.storage.iter().copied() {
            let source = self.source.storage(source_id)?;
            let destination = self.destination.allocate_storage(|id| MirStorage {
                id,
                source: None,
                name: source.name.clone(),
                kind,
                ty: source.ty,
                span: source.span,
            })?;
            self.maps.storage.entries.insert(source_id, destination);
        }
        Ok(())
    }

    fn allocate_values(&mut self) -> Result<(), MirRewriteError> {
        for source_id in self.request.values.iter().copied() {
            let source = self.source.value(source_id)?;
            let destination = self.destination.allocate_value(|id| MirValue {
                id,
                ty: source.ty,
                span: source.span,
            })?;
            self.maps.values.entries.insert(source_id, destination);
        }
        Ok(())
    }

    fn allocate_blocks(&mut self) -> Result<(), MirRewriteError> {
        let mut placement = self.request.block_placement;
        for source_id in self.request.blocks.iter().copied() {
            let source = self.source.block(source_id)?;
            let destination = self
                .destination
                .allocate_block(placement, |id| MirBasicBlock {
                    id,
                    instructions: Vec::new(),
                    terminator: None,
                    span: source.span,
                })?;
            self.maps.blocks.entries.insert(source_id, destination);
            placement = BlockPlacement::After(destination);
        }
        Ok(())
    }

    fn allocate_guards(&mut self) {
        for source in self.request.optional_guards.iter().copied() {
            let destination = self.destination.allocate_optional_guard();
            self.maps
                .optional_guards
                .entries
                .insert(source, destination);
        }
    }

    fn allocate_path_conditions(&mut self) -> Result<(), MirRewriteError> {
        for source_id in self.request.path_conditions.iter().copied() {
            let source = self.source.path_condition(source_id)?;
            let site = MirLocalIdentitySite::PathCondition(source_id.index());
            let mapper = RehomeMapper::new(self.source.callable, &self.maps);
            let parent = source
                .parent
                .map(|identity| mapper.path_condition(site, identity))
                .transpose()?;
            let activation = mapper.storage(site, source.activation)?;
            let active_predecessor = mapper.block(site, source.active_predecessor)?;
            let inactive_predecessor = mapper.block(site, source.inactive_predecessor)?;
            let merge = mapper.block(site, source.merge)?;
            let destination = self
                .destination
                .allocate_path_condition(|id| MirPathCondition {
                    id,
                    parent,
                    activation,
                    active_predecessor,
                    inactive_predecessor,
                    merge,
                    span: source.span,
                })?;
            self.maps
                .path_conditions
                .entries
                .insert(source_id, destination);
        }
        Ok(())
    }

    fn clone_blocks(&mut self) -> Result<(), MirRewriteError> {
        let mut mapper = RehomeMapper::new(self.source.callable, &self.maps);
        for source_id in self.request.blocks.iter().copied() {
            let source = self.source.block(source_id)?;
            let destination = self.maps.blocks.destination(source_id)?;
            let mut instructions = source.instructions.clone();
            for (instruction, entry) in instructions.iter_mut().enumerate() {
                map_instruction(
                    entry,
                    &mut mapper,
                    MirLocalIdentitySite::Instruction {
                        block: source_id.index(),
                        instruction,
                    },
                )?;
            }
            let mut terminator = source.terminator.clone();
            if let Some(terminator) = &mut terminator {
                map_terminator(
                    terminator,
                    &mut mapper,
                    MirLocalIdentitySite::Terminator(source_id.index()),
                )?;
            }
            self.destination
                .rewrite_block_instructions(destination, |_| instructions)?;
            self.destination
                .rewrite_block_terminator(destination, |_| terminator)?;
        }
        Ok(())
    }

    fn clone_path_conditions(&mut self) -> Result<(), MirRewriteError> {
        let mut mapper = RehomeMapper::new(self.source.callable, &self.maps);
        for source_id in self.request.path_conditions.iter().copied() {
            let destination = self.maps.path_conditions.destination(source_id)?;
            let mut condition = self.source.path_condition(source_id)?.clone();
            map_path_condition_metadata(
                &mut condition,
                &mut mapper,
                MirLocalIdentitySite::PathCondition(source_id.index()),
            )?;
            self.destination
                .replace_imported_path_condition(destination, condition)?;
        }
        Ok(())
    }

    fn clone_logical_records(
        &mut self,
    ) -> Result<Vec<(usize, LogicalRecordIndex)>, MirRewriteError> {
        let mut mapper = RehomeMapper::new(self.source.callable, &self.maps);
        let mut imported = Vec::with_capacity(self.request.logical_records.len());
        for source_index in self.request.logical_records.iter().copied() {
            let mut expression = self.source.logical_record(source_index)?.clone();
            map_logical_expression(
                &mut expression,
                &mut mapper,
                MirLocalIdentitySite::LogicalExpression(source_index),
            )?;
            let destination = self.destination.allocate_logical_record(expression);
            imported.push((source_index, destination));
        }
        Ok(imported)
    }
}
