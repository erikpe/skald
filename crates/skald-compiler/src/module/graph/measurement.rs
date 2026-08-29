//! Optional accounting for the loader's real filesystem and frontend work.

use super::model::{ModuleGraph, ModuleGraphLoadFailure};
use crate::module::ModulePath;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModuleParseStage {
    Discovery,
    Final,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModuleParseMeasurement {
    module: ModulePath,
    stage: ModuleParseStage,
    tokens: u64,
    completed: bool,
}

impl ModuleParseMeasurement {
    pub(crate) const fn module(&self) -> &ModulePath {
        &self.module
    }

    pub(crate) const fn stage(&self) -> ModuleParseStage {
        self.stage
    }

    pub(crate) const fn tokens(&self) -> u64 {
        self.tokens
    }

    pub(crate) const fn completed(&self) -> bool {
        self.completed
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ModuleLoadMeasurementOptions {
    details: bool,
    trace: bool,
}

impl ModuleLoadMeasurementOptions {
    pub(crate) const fn new(details: bool, trace: bool) -> Self {
        Self { details, trace }
    }

    const fn enabled(self) -> bool {
        self.details || self.trace
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct ModuleLoadMeasurements {
    enabled: bool,
    trace: bool,
    reached_modules: u64,
    source_reads: u64,
    source_bytes: u64,
    discovery_lex_executions: u64,
    discovery_parse_executions: u64,
    discovery_tokens: u64,
    final_lex_executions: u64,
    final_parse_executions: u64,
    final_tokens: u64,
    parses: Vec<ModuleParseMeasurement>,
}

impl ModuleLoadMeasurements {
    pub(super) fn new(options: ModuleLoadMeasurementOptions) -> Self {
        Self {
            enabled: options.enabled(),
            trace: options.trace,
            ..Self::default()
        }
    }

    pub(super) fn record_source_read(&mut self, bytes: Option<usize>) {
        if !self.enabled {
            return;
        }
        self.source_reads = self.source_reads.saturating_add(1);
        if let Some(bytes) = bytes {
            self.reached_modules = self.reached_modules.saturating_add(1);
            self.source_bytes = self.source_bytes.saturating_add(count(bytes));
        }
    }

    pub(super) fn record_lex(&mut self, stage: ModuleParseStage, tokens: usize) {
        if !self.enabled {
            return;
        }
        let tokens = count(tokens);
        match stage {
            ModuleParseStage::Discovery => {
                self.discovery_lex_executions = self.discovery_lex_executions.saturating_add(1);
                self.discovery_tokens = self.discovery_tokens.saturating_add(tokens);
            }
            ModuleParseStage::Final => {
                self.final_lex_executions = self.final_lex_executions.saturating_add(1);
                self.final_tokens = self.final_tokens.saturating_add(tokens);
            }
        }
    }

    pub(super) fn record_parse(
        &mut self,
        module: &ModulePath,
        stage: ModuleParseStage,
        tokens: usize,
        completed: bool,
    ) {
        if !self.enabled {
            return;
        }
        match stage {
            ModuleParseStage::Discovery => {
                self.discovery_parse_executions = self.discovery_parse_executions.saturating_add(1);
            }
            ModuleParseStage::Final => {
                self.final_parse_executions = self.final_parse_executions.saturating_add(1);
            }
        }
        if self.trace {
            self.parses.push(ModuleParseMeasurement {
                module: module.clone(),
                stage,
                tokens: count(tokens),
                completed,
            });
        }
    }

    pub(crate) const fn reached_modules(&self) -> u64 {
        self.reached_modules
    }

    pub(crate) const fn source_reads(&self) -> u64 {
        self.source_reads
    }

    pub(crate) const fn source_bytes(&self) -> u64 {
        self.source_bytes
    }

    pub(crate) const fn discovery_lex_executions(&self) -> u64 {
        self.discovery_lex_executions
    }

    pub(crate) const fn discovery_parse_executions(&self) -> u64 {
        self.discovery_parse_executions
    }

    pub(crate) const fn discovery_tokens(&self) -> u64 {
        self.discovery_tokens
    }

    pub(crate) const fn final_lex_executions(&self) -> u64 {
        self.final_lex_executions
    }

    pub(crate) const fn final_parse_executions(&self) -> u64 {
        self.final_parse_executions
    }

    pub(crate) const fn final_tokens(&self) -> u64 {
        self.final_tokens
    }

    pub(crate) fn parses(&self) -> &[ModuleParseMeasurement] {
        &self.parses
    }
}

pub(crate) struct MeasuredModuleGraphLoad {
    pub(crate) result: Result<ModuleGraph, ModuleGraphLoadFailure>,
    pub(crate) measurements: ModuleLoadMeasurements,
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
