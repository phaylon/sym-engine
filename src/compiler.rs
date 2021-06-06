
use std::sync::{Arc};
use crate::{ast};

mod cfg;

pub struct CompiledRule {
}

#[derive(Debug)]
pub enum CompileError {
    IllegalWildcard {
        line: u32,
    },
    IllegalNamedBinding {
        line: u32,
        name: Arc<str>,
    },
    IllegalBindingMatch {
        line: u32,
        name: Arc<str>,
    },
    RepeatBindings {
        names: Vec<Arc<str>>,
    },
    SingleBindingUse {
        names: Vec<Arc<str>>,
    },
    IllegalReuse {
        line: u32,
        name: Arc<str>,
    },
    IllegalNewBinding {
        line: u32,
        name: Arc<str>,
    },
    IllegalRemoval {
        line: u32,
    },
    IllegalEnumSpecification {
        line: u32,
    },
    IllegalObjectSpecification {
        line: u32,
    },
}

pub fn compile(
    ast: &ast::Rule<'_>,
    input_variables: &[Arc<str>],
) -> Result<CompiledRule, CompileError> {
    dbg!(ast);
    let cfg = cfg::ast_to_cfg(ast, input_variables)?;
    dbg!(&cfg);
    todo!()
}
