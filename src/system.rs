
use std::sync::{Arc};
use std::path::{Path};
use std::io::{Error as IoError};
use crate::{parser, compiler, runtime, Id, Value, Access};

#[derive(Debug)]
pub struct System {
    name: Arc<str>,
    input_variables: Vec<Arc<str>>,
    max_binding_len: usize,
    rules: Vec<compiler::CompiledRule>,
    #[cfg(feature = "tracing")]
    tracing_span: tracing::Span,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SystemError {
    #[error("invalid system name `{0}`")]
    InvalidName(Arc<str>),
    #[error("invalid input variable name `${0}`")]
    InvalidInputVariable(Arc<str>),
    #[error("duplicate input variable name `${0}`")]
    DuplicateInputVariable(Arc<str>),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RuntimeError {
    #[error("stopped after {count} rule firings")]
    Stopped {
        count: u64,
    },
    #[error("expected {expected} input arguments but received {received}")]
    InvalidInputArgumentLen {
        expected: usize,
        received: usize,
    },
}

impl System {

    pub fn new(name: &str, input_variables: &[&str]) -> Result<Self, SystemError> {
        if !parser::is_path(name) {
            return Err(SystemError::InvalidName(name.into()));
        }
        let mut check_vars = input_variables;
        while let Some((&curr, rest)) = check_vars.split_first() {
            check_vars = rest;
            if !parser::is_variable_ident(curr) {
                return Err(SystemError::InvalidInputVariable(curr.into()));
            }
            if rest.contains(&curr) {
                return Err(SystemError::DuplicateInputVariable(curr.into()));
            }
        }
        Ok(Self {
            name: name.into(),
            input_variables: input_variables.iter().map(|&var| var.into()).collect(),
            max_binding_len: input_variables.len(),
            rules: Vec::new(),
            #[cfg(feature = "tracing")]
            tracing_span: tracing::debug_span!("system", system_name = name),
        })
    }

    pub fn name(&self) -> &Arc<str> {
        &self.name
    }

    pub fn input_variables(&self) -> &[Arc<str>] {
        &self.input_variables
    }

    pub fn count(&self) -> usize {
        self.rules.len()
    }

    pub fn build_rule<F>(&mut self, name: &str, builder_cb: F) -> Result<(), LoadError>
    where
        F: for<'seq, 'bind> FnOnce(
            crate::SelectBuilder<'seq, 'bind>,
            &[crate::BuilderBinding<'bind>],
        ) -> crate::ApplyBuilder<'seq, 'bind>,
    {
        let compiled_rule = compiler::build_and_compile(
            name.into(),
            self.input_variables(),
            builder_cb,
        );
        self.load(compiled_rule)
    }

    fn load(&mut self, rule: compiler::CompiledRule) -> Result<(), LoadError> {
        if self.rules.iter().any(|ex| ex.name() == rule.name()) {
            return Err(LoadError::DuplicateRuleName(self.name.clone(), rule.name().clone()));
        }
        if rule.bindings_len() > self.max_binding_len {
            self.max_binding_len = rule.bindings_len();
        }
        self.rules.push(rule);
        Ok(())
    }

    fn make_bindings_storage(&self, inputs: &[Id]) -> Result<Vec<Value>, RuntimeError> {
        self.verify_inputs(inputs)?;
        let rest_bindings_len = self.max_binding_len
            .checked_sub(inputs.len())
            .expect("less inputs than max bindings");
        let bindings = inputs
            .iter()
            .map(|id| Value::Object(*id))
            .chain((0..rest_bindings_len).map(|_| Value::Int(0)))
            .collect();
        Ok(bindings)
    }

    fn verify_inputs(&self, inputs: &[Id]) -> Result<(), RuntimeError> {
        if inputs.len() == self.input_variables.len() {
            Ok(())
        } else {
            Err(RuntimeError::InvalidInputArgumentLen {
                expected: self.input_variables.len(),
                received: inputs.len(),
            })
        }
    }

    pub fn run_to_first(
        &self,
        space: &mut dyn Access,
        inputs: &[Id],
    ) -> Result<Option<Arc<str>>, RuntimeError> {

        #[cfg(feature = "tracing")]
        let _enter = self.tracing_span.enter();

        #[cfg(feature = "tracing")]
        tracing::trace!(system_run_mode = "to-first");

        let mut bindings = self.make_bindings_storage(inputs)?;
        for rule in &self.rules {
            let rule_fired = runtime::attempt_rule_firing(rule, space, &mut bindings);
            if rule_fired {
                return Ok(Some(rule.name().clone()));
            }
        }
        Ok(None)
    }

    pub fn run_rule_saturation(
        &self,
        space: &mut dyn Access,
        inputs: &[Id],
    ) -> Result<u64, RuntimeError> {
        self.run_rule_saturation_with_control(
            space,
            inputs,
            |_, _, _| RuntimeControl::Continue,
        )
    }

    pub fn run_rule_saturation_with_control<F>(
        &self,
        space: &mut dyn Access,
        inputs: &[Id],
        mut control: F,
    ) -> Result<u64, RuntimeError>
    where
        F: FnMut(&Arc<str>, &dyn Access, u64) -> RuntimeControl,
    {
        #[cfg(feature = "tracing")]
        let _enter = self.tracing_span.enter();

        #[cfg(feature = "tracing")]
        tracing::trace!(system_run_mode = "rule-saturation");

        let mut run_count = 0;
        let mut bindings = self.make_bindings_storage(inputs)?;
        for rule in &self.rules {
            'current_rule: loop {
                let rule_fired = runtime::attempt_rule_firing(rule, space, &mut bindings);
                if rule_fired {
                    run_count += 1;
                    match control(rule.name(), space, run_count) {
                        RuntimeControl::Continue => {
                            continue 'current_rule;
                        },
                        RuntimeControl::Stop => {

                            #[cfg(feature = "tracing")]
                            tracing::debug!("system stopped");

                            return Err(RuntimeError::Stopped { count: run_count });
                        },
                    }
                }
                break 'current_rule;
            }
        }
        Ok(run_count)
    }

    pub fn run_saturation(
        &self,
        space: &mut dyn Access,
        inputs: &[Id],
    ) -> Result<u64, RuntimeError> {
        self.run_saturation_with_control(
            space,
            inputs,
            |_, _, _| RuntimeControl::Continue,
        )
    }

    pub fn run_saturation_with_control<F>(
        &self,
        space: &mut dyn Access,
        inputs: &[Id],
        mut control: F,
    ) -> Result<u64, RuntimeError>
    where
        F: FnMut(&Arc<str>, &dyn Access, u64) -> RuntimeControl,
    {
        #[cfg(feature = "tracing")]
        let _enter = self.tracing_span.enter();

        #[cfg(feature = "tracing")]
        tracing::trace!(system_run_mode = "saturation");

        let mut run_count = 0;
        let mut bindings = self.make_bindings_storage(inputs)?;
        'firing: loop {
            for rule in &self.rules {
                let rule_fired = runtime::attempt_rule_firing(rule, space, &mut bindings);
                if rule_fired {
                    run_count += 1;
                    match control(rule.name(), space, run_count) {
                        RuntimeControl::Continue => {
                            continue 'firing;
                        },
                        RuntimeControl::Stop => {

                            #[cfg(feature = "tracing")]
                            tracing::debug!("system stopped");

                            return Err(RuntimeError::Stopped { count: run_count });
                        },
                    }
                }
            }
            break 'firing;
        }
        Ok(run_count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeControl {
    Continue,
    Stop,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum LoadError {
    #[error("unable to parse source code:\n{0}")]
    Parse(String),
    #[error("rule compilation failed")]
    Compile(#[source] compiler::CompileError),
    #[error("duplicate rule declaration for system `{0}` rule `{1}`")]
    DuplicateRuleName(Arc<str>, Arc<str>),
    #[error("unknown system `{0}`")]
    NoSuchSystem(Arc<str>),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("source file `{path}` could not be loaded")]
pub struct FileLoadError {
    pub path: Arc<Path>,
    #[source]
    pub kind: FileLoadErrorKind,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum FileLoadErrorKind {
    #[error(transparent)]
    Read(Arc<IoError>),
    #[error(transparent)]
    Load(LoadError),
}

pub struct SystemLoader<'a> {
    systems: Vec<&'a mut System>,
}

impl<'a> SystemLoader<'a> {

    pub fn new(systems: Vec<&'a mut System>) -> Self {
        Self { systems }
    }

    pub fn load_file<P>(&mut self, path: P) -> Result<usize, FileLoadError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|error| FileLoadError {
                path: path.into(),
                kind: FileLoadErrorKind::Read(error.into()),
            })?;
        self.load_str(&contents)
            .map_err(|error| FileLoadError {
                path: path.into(),
                kind: FileLoadErrorKind::Load(error),
            })
    }

    pub fn load_str(&mut self, contents: &str) -> Result<usize, LoadError> {
        let parsed_rules = parser::parse(contents)
            .map_err(LoadError::Parse)?;
        let rule_count = parsed_rules.len();
        'rules: for rule in parsed_rules {
            for system in &mut self.systems {
                if system.name().as_ref() == rule.system_name.as_str() {
                    let compiled = compiler::compile(&rule, system.input_variables())
                        .map_err(LoadError::Compile)?;
                    system.load(compiled)?;
                    continue 'rules;
                }
            }
            return Err(LoadError::NoSuchSystem(rule.system_name.as_str().into()));
        }
        Ok(rule_count)
    }
}

pub fn control_limit_total(total_limit: u64)
-> impl FnMut(&Arc<str>, &dyn Access, u64) -> RuntimeControl
{
    move |_name, _, total_count| {
        if total_count >= total_limit {

            #[cfg(feature = "tracing")]
            tracing::warn!("rule `{}` exceeded total limit of {}", &_name, total_limit);

            RuntimeControl::Stop
        } else {
            RuntimeControl::Continue
        }
    }
}

pub fn control_limit_per_rule(per_rule_limit: u64)
-> impl FnMut(&Arc<str>, &dyn Access, u64) -> RuntimeControl
{
    let mut per_rule_counts = std::collections::HashMap::new();
    move |name, _, _total_count| {
        let rule_count_entry = per_rule_counts.entry(name.clone()).or_insert(0u64);
        *rule_count_entry += 1;
        if *rule_count_entry >= per_rule_limit {

            #[cfg(feature = "tracing")]
            tracing::warn!(
                "rule `{}` exceeded per-rule limit of {} (total count: {})",
                &name,
                per_rule_limit,
                _total_count,
            );

            RuntimeControl::Stop
        } else {
            RuntimeControl::Continue
        }
    }
}

pub fn control_limit_total_and_per_rule(total_limit: u64, per_rule_limit: u64)
-> impl FnMut(&Arc<str>, &dyn Access, u64) -> RuntimeControl
{
    let mut cb_control_total = control_limit_total(total_limit);
    let mut cb_control_per_rule = control_limit_per_rule(per_rule_limit);
    move |name, access, total_count| {
        if let RuntimeControl::Continue = cb_control_total(name, access, total_count) {
            cb_control_per_rule(name, access, total_count)
        } else {
            RuntimeControl::Stop
        }
    }
}