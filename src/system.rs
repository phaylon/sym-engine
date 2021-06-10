
use std::sync::{Arc};
use std::path::{Path};
use std::io::{Error as IoError};
use crate::{parser, compiler, runtime, Id, Value, Access};

pub struct System {
    name: Arc<str>,
    input_variables: Vec<Arc<str>>,
    max_binding_len: usize,
    rules: Vec<compiler::CompiledRule>,
}

#[derive(Debug, Clone)]
pub enum SystemError {
    InvalidName(Arc<str>),
    InvalidInputVariable(Arc<str>),
    DuplicateInputVariable(Arc<str>),
}

#[derive(Debug, Clone)]
pub enum RuntimeError {
    Stopped {
        count: u64,
    },
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

    fn load(&mut self, rule: compiler::CompiledRule) -> Result<(), LoadError> {
        if self.rules.iter().any(|ex| ex.name() == rule.name()) {
            return Err(LoadError::DuplicateRuleName(rule.name().clone()));
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

#[derive(Debug, Clone)]
pub enum LoadError {
    Parse(String),
    Compile(compiler::CompileError),
    DuplicateRuleName(Arc<str>),
    NoSuchSystem(Arc<str>),
}

#[derive(Debug, Clone)]
pub struct FileLoadError {
    pub path: Arc<Path>,
    pub kind: FileLoadErrorKind,
}

#[derive(Debug, Clone)]
pub enum FileLoadErrorKind {
    Read(Arc<IoError>),
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