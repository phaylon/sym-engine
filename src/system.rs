
use std::sync::{Arc};
use crate::{parser, compiler};

pub struct System {
    name: Arc<str>,
    input_variables: Vec<Arc<str>>,
    max_binding_len: usize,
    rules: Vec<compiler::CompiledRule>,
}

#[derive(Debug)]
pub enum SystemError {
    InvalidName(Arc<str>),
    InvalidInputVariable(Arc<str>),
    DuplicateInputVariable(Arc<str>),
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
}

#[derive(Debug)]
pub enum LoadError {
    Parse(String),
    Compile(compiler::CompileError),
    DuplicateRuleName(Arc<str>),
}

pub struct SystemLoader<'a> {
    systems: Vec<&'a mut System>,
}

impl<'a> SystemLoader<'a> {

    pub fn new(systems: Vec<&'a mut System>) -> Self {
        Self { systems }
    }

    pub fn load_str(&mut self, contents: &str) -> Result<usize, LoadError> {
        let parsed_rules = parser::parse(contents)
            .map_err(LoadError::Parse)?;
        for rule in parsed_rules {
            for system in &mut self.systems {
                if system.name().as_ref() == rule.system_name.as_str() {
                    let compiled = compiler::compile(&rule, system.input_variables())
                        .map_err(LoadError::Compile)?;
                    dbg!(&compiled);
                    system.load(compiled)?;
                }
            }
        }
        todo!()
    }
}