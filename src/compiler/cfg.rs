
use std::sync::{Arc};
use std::cell::{RefCell};
use fnv::{FnvHashSet};
use std::collections::{HashMap};
use crate::{ast, Symbol, Value};
use crate::data::{CompareOp};
use crate::parser::{Span};
use super::{
    CompileError,
    EnumOption,
    OpenTupleItem,
    Calculation,
    CompareValue,
    OpApply,
    ApplyTupleItem,
    Binding,
    BindingSequence,
};

pub fn ast_to_cfg(
    ast: &ast::Rule<'_>,
    input_variables: &[Arc<str>],
) -> Result<CfgRule, CompileError> {

    let binding_sequence = BindingSequence::new();
    let instance_counts = RefCell::new(HashMap::new());
    let access_counts = RefCell::new(HashMap::new());
    let binding_names = RefCell::new(HashMap::new());
    let mut env = Env::new(&binding_sequence, &instance_counts, &access_counts, &binding_names);

    for variable in input_variables {
        env.bind(variable);
    }

    let mut select = Vec::new();
    compile_rule_selects(&mut env, &ast.select, &mut select)?;

    let mut apply = Vec::new();
    compile_rule_applys(&mut env, &ast.apply, &mut apply)?;

    verify_distinct_bindings(&instance_counts.borrow())?;
    verify_multi_usage(&env, &access_counts.borrow(), input_variables.len())?;

    let cfg_rule = CfgRule {
        name: ast.name.as_str().into(),
        select: select,
        apply: apply,
        bindings_len: binding_sequence.len(),
    };
    Ok(cfg_rule)
}

#[derive(Debug, Clone)]
pub struct CfgRule {
    pub name: Arc<str>,
    pub select: Vec<CfgOpSelect>,
    pub apply: Vec<OpApply>,
    pub bindings_len: usize,
}

#[derive(Debug, Clone)]
struct Env<'a> {
    binding_sequence: &'a BindingSequence,
    visible_bindings: HashMap<String, Binding>,
    instance_counts: &'a RefCell<HashMap<String, usize>>,
    access_counts: &'a RefCell<HashMap<Binding, usize>>,
    binding_names: &'a RefCell<HashMap<Binding, String>>,
}

impl<'a> Env<'a> {

    fn new(
        binding_sequence: &'a BindingSequence,
        instance_counts: &'a RefCell<HashMap<String, usize>>,
        access_counts: &'a RefCell<HashMap<Binding, usize>>,
        binding_names: &'a RefCell<HashMap<Binding, String>>,
    ) -> Self {
        Self {
            binding_sequence,
            instance_counts,
            access_counts,
            binding_names,
            visible_bindings: HashMap::new(),
        }
    }

    fn binding_set(&self) -> FnvHashSet<Binding> {
        self.visible_bindings.values().copied().collect()
    }

    fn bind(&mut self, name: &str) -> Binding {
        if let Some(binding) = self.visible_bindings.get(name).copied() {
            *self.access_counts.borrow_mut().entry(binding).or_insert(0) += 1;
            binding
        } else {
            let binding = self.binding_sequence.next();
            self.visible_bindings.insert(name.into(), binding);
            *self.instance_counts.borrow_mut().entry(name.into()).or_insert(0) += 1;
            *self.access_counts.borrow_mut().entry(binding).or_insert(0) += 1;
            self.binding_names.borrow_mut().entry(binding).or_insert_with(|| name.into());
            binding
        }
    }

    fn bind_new(&mut self, name: &str) -> Option<Binding> {
        if self.visible_bindings.contains_key(name) {
            None
        } else {
            Some(self.bind(name))
        }
    }

    fn find(&mut self, name: &str) -> Option<Binding> {
        if self.visible_bindings.contains_key(name) {
            Some(self.bind(name))
        } else {
            None
        }
    }

    fn anon(&mut self) -> Binding {
        self.binding_sequence.next()
    }

    fn binding_name(&self, binding: Binding) -> Option<String> {
        self.binding_names.borrow().get(&binding).cloned()
    }
}

fn verify_multi_usage(
    env: &Env<'_>,
    access_counts: &HashMap<Binding, usize>,
    input_variables_len: usize,
) -> Result<(), CompileError> {

    let mut single_use = Vec::new();
    for (binding, count) in access_counts {
        if *count == 1 && !(0..input_variables_len).contains(&binding.index()) {
            if let Some(name) = env.binding_name(*binding) {
                single_use.push(name.into());
            }
        }
    }

    if !single_use.is_empty() {
        Err(CompileError::SingleBindingUse {
            names: single_use,
        })
    } else {
        Ok(())
    }
}

fn verify_distinct_bindings(
    instance_counts: &HashMap<String, usize>,
) -> Result<(), CompileError> {

    let repeated_bindings = instance_counts
        .iter()
        .filter_map(|(name, count)| {
            if *count > 1 {
                Some(name.as_str().into())
            } else {
                None
            }
        })
        .collect::<Vec<Arc<str>>>();

    if !repeated_bindings.is_empty() {
        return Err(CompileError::RepeatBindings {
            names: repeated_bindings,
        });
    }

    Ok(())
}

fn compile_rule_applys(
    env: &mut Env<'_>,
    rule_applys: &[ast::RuleApply<'_>],
    ops: &mut Vec<OpApply>,
) -> Result<(), CompileError> {
    for rule_apply in rule_applys {
        compile_rule_apply(env, rule_apply, ops)?;
    }
    Ok(())
}

fn compile_rule_apply(
    env: &mut Env<'_>,
    rule_apply: &ast::RuleApply<'_>,
    ops: &mut Vec<OpApply>,
) -> Result<(), CompileError> {
    match rule_apply {
        ast::RuleApply::Remove(spec) => compile_apply_remove(env, spec, ops),
        ast::RuleApply::Add(spec) => compile_apply_add(env, spec, ops),
    }
}

fn compile_apply_add(
    env: &mut Env<'_>,
    spec: &ast::BindingAttributeSpec<'_>,
    ops: &mut Vec<OpApply>,
) -> Result<(), CompileError> {
    let binding = existing_named_binding(env, &spec.variable, &spec.position)?;
    compile_apply_add_attribute(env, binding, &spec.attribute_spec, ops)
}

fn compile_apply_add_attribute(
    env: &mut Env<'_>,
    binding: Binding,
    spec: &ast::AttributeSpec<'_>,
    ops: &mut Vec<OpApply>,
) -> Result<(), CompileError> {
    match &spec.value_spec.kind {
        ast::ValueSpecKind::Literal(literal) => {
            ops.push(OpApply::AddValueAttribute {
                binding,
                attribute: spec.attribute.as_str().into(),
                value: literal.to_value(),
            });
            Ok(())
        },
        ast::ValueSpecKind::Variable(variable) => {
            let value_binding = existing_named_binding(env, variable, &spec.position)?;
            ops.push(OpApply::AddBindingAttribute {
                binding,
                attribute: spec.attribute.as_str().into(),
                value_binding,
            });
            Ok(())
        },
        ast::ValueSpecKind::Tuple(ast::Bindable { variable: direct, inner: values }) => {
            let value_binding = nameable_new_binding(env, direct, &spec.position)?;
            compile_apply_tuple(env, value_binding, values, true, ops)?;
            ops.push(OpApply::AddBindingAttribute {
                binding,
                attribute: spec.attribute.as_str().into(),
                value_binding,
            });
            Ok(())
        },
        ast::ValueSpecKind::Enum(_) => Err(CompileError::IllegalEnumSpecification {
            line: spec.position.location_line(),
        }),
        ast::ValueSpecKind::Struct(ast::Bindable { variable: direct, inner: attributes }) => {
            let value_binding = nameable_new_binding(env, direct, &spec.position)?;
            compile_apply_object(env, value_binding, attributes, ops)?;
            ops.push(OpApply::AddBindingAttribute {
                binding,
                attribute: spec.attribute.as_str().into(),
                value_binding,
            });
            Ok(())
        },
    }
}

fn compile_apply_remove(
    env: &mut Env<'_>,
    spec: &ast::BindingAttributeSpec<'_>,
    ops: &mut Vec<OpApply>,
) -> Result<(), CompileError> {
    let binding = existing_named_binding(env, &spec.variable, &spec.position)?;
    match &spec.attribute_spec.value_spec.kind {
        ast::ValueSpecKind::Literal(literal) => {
            ops.push(OpApply::RemoveValueAttribute {
                binding,
                attribute: spec.attribute_spec.attribute.as_str().into(),
                value: literal.to_value(),
            });
            Ok(())
        },
        ast::ValueSpecKind::Variable(variable) => {
            let value_binding = existing_named_binding(env, variable, &spec.position)?;
            ops.push(OpApply::RemoveBindingAttribute {
                binding,
                attribute: spec.attribute_spec.attribute.as_str().into(),
                value_binding,
            });
            Ok(())
        },
        ast::ValueSpecKind::Tuple(ast::Bindable { variable: direct, inner: values }) => {
            let value_binding = nameable_new_binding(env, direct, &spec.position)?;
            compile_apply_tuple(env, value_binding, values, false, ops)?;
            ops.push(OpApply::RemoveBindingAttribute {
                binding,
                attribute: spec.attribute_spec.attribute.as_str().into(),
                value_binding,
            });
            Ok(())
        },
        ast::ValueSpecKind::Enum(_) => Err(CompileError::IllegalEnumSpecification {
            line: spec.position.location_line(),
        }),
        ast::ValueSpecKind::Struct(_) => Err(CompileError::IllegalObjectSpecification {
            line: spec.position.location_line(),
        }),
    }
}

fn compile_apply_object(
    env: &mut Env<'_>,
    binding: Binding,
    attributes: &[ast::AttributeSpec<'_>],
    ops: &mut Vec<OpApply>,
) -> Result<(), CompileError> {
    ops.push(OpApply::CreateObject {
        binding,
    });
    for attribute in attributes {
        compile_apply_add_attribute(env, binding, attribute, ops)?;
    }
    Ok(())
}

fn compile_apply_tuple(
    env: &mut Env<'_>,
    binding: Binding,
    values: &[ast::ValueSpec<'_>],
    allow_object_construction: bool,
    ops: &mut Vec<OpApply>,
) -> Result<(), CompileError> {
    let mut cfg_tuple_items = Vec::new();
    for value_spec in values {
        match &value_spec.kind {
            ast::ValueSpecKind::Literal(literal) => {
                cfg_tuple_items.push(ApplyTupleItem::Value(literal.to_value()));
            },
            ast::ValueSpecKind::Variable(variable) => {
                let value_binding = existing_named_binding(env, variable, &value_spec.position)?;
                cfg_tuple_items.push(ApplyTupleItem::Binding(value_binding));
            },
            ast::ValueSpecKind::Tuple(ast::Bindable { variable: direct, inner: values }) => {
                let value_binding = nameable_new_binding(env, direct, &value_spec.position)?;
                compile_apply_tuple(env, value_binding, values, allow_object_construction, ops)?;
                cfg_tuple_items.push(ApplyTupleItem::Binding(value_binding));
            },
            ast::ValueSpecKind::Struct(ast::Bindable { variable: direct, inner: attributes }) => {
                if allow_object_construction {
                    let value_binding = nameable_new_binding(env, direct, &value_spec.position)?;
                    compile_apply_object(env, value_binding, attributes, ops)?;
                    cfg_tuple_items.push(ApplyTupleItem::Binding(value_binding));
                } else {
                    return Err(CompileError::IllegalObjectSpecification {
                        line: value_spec.position.location_line(),
                    });
                }
            },
            ast::ValueSpecKind::Enum(_) => {
                return Err(CompileError::IllegalEnumSpecification {
                    line: value_spec.position.location_line(),
                });
            },
        }
    }
    ops.push(OpApply::CreateTuple {
        binding,
        items: cfg_tuple_items,
    });
    Ok(())
}

fn existing_named_binding_with_name(
    env: &mut Env<'_>,
    variable: &ast::Variable<'_>,
    position: &Span<'_>,
) -> Result<(Binding, Arc<str>), CompileError> {
    if let Some(name) = variable.as_str() {
        if let Some(binding) = env.find(name) {
            Ok((binding, name.into()))
        } else {
            Err(CompileError::IllegalNewBinding {
                line: position.location_line(),
                name: name.into(),
            })
        }
    } else {
        Err(CompileError::IllegalWildcard {
            line: position.location_line(),
        })
    }
}

fn existing_named_binding(
    env: &mut Env<'_>,
    variable: &ast::Variable<'_>,
    position: &Span<'_>,
) -> Result<Binding, CompileError> {
    if let Some(name) = variable.as_str() {
        if let Some(binding) = env.find(name) {
            Ok(binding)
        } else {
            Err(CompileError::IllegalNewBinding {
                line: position.location_line(),
                name: name.into(),
            })
        }
    } else {
        Err(CompileError::IllegalWildcard {
            line: position.location_line(),
        })
    }
}

fn nameable_new_binding(
    env: &mut Env<'_>,
    variable: &ast::Variable<'_>,
    position: &Span<'_>,
) -> Result<Binding, CompileError> {
    if let Some(name) = variable.as_str() {
        if let Some(binding) = env.bind_new(name) {
            Ok(binding)
        } else {
            Err(CompileError::IllegalReuse {
                line: position.location_line(),
                name: name.into(),
            })
        }
    } else {
        Ok(env.anon())
    }
}

fn compile_rule_selects(
    env: &mut Env<'_>,
    rule_selects: &[ast::RuleSelect<'_>],
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    for rule_select in rule_selects {
        compile_rule_select(env, rule_select, ops)?;
    }
    Ok(())
}

fn compile_rule_select(
    env: &mut Env<'_>,
    rule_select: &ast::RuleSelect<'_>,
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    match rule_select {
        ast::RuleSelect::Binding(spec) => {
            compile_select_toplevel_binding(
                env,
                &spec.variable,
                &spec.position,
                &spec.value_spec,
                ops,
            )
        },
        ast::RuleSelect::BindingAttribute(spec) => {
            let binding = existing_named_binding(env, &spec.variable, &spec.position)?;
            compile_select_attribute(
                env,
                binding,
                &spec.attribute_spec,
                &spec.position,
                ops,
            )
        },
        ast::RuleSelect::Not(sub_selects) => {
            let mut sub_ops = Vec::new();
            let mut sub_env = env.clone();
            compile_rule_selects(&mut sub_env, sub_selects, &mut sub_ops)?;
            ops.push(CfgOpSelect::Not {
                inherited_bindings: env.binding_set(),
                body: sub_ops,
            });
            Ok(())
        },
        ast::RuleSelect::Comparison(comparison) => {
            compile_select_comparison(env, comparison, ops)
        },
        ast::RuleSelect::Calculation(variable, calculation, position) => {
            let result_binding = named_new_binding(env, variable, position)?;
            let operation = compile_calculation(env, position, calculation)?;
            ops.push(CfgOpSelect::Calculation {
                result_binding,
                operation,
            });
            Ok(())
        },
    }
}

fn compile_calculation(
    env: &mut Env,
    position: &Span<'_>,
    calculation: &ast::Calculation<'_>,
) -> Result<Calculation, CompileError> {
    match calculation {
        ast::Calculation::Int(value) =>
            Ok(Calculation::Value(Value::from(*value))),
        ast::Calculation::Float(value) =>
            Ok(Calculation::Value(Value::from(*value))),
        ast::Calculation::Variable(variable) =>
            Ok(Calculation::Binding(existing_named_binding(env, variable, position)?)),
        ast::Calculation::BimOp(op, left, right) =>
            Ok(Calculation::BinOp(
                *op,
                Box::new(compile_calculation(env, position, left)?),
                Box::new(compile_calculation(env, position, right)?),
            )),
    }
}

fn compile_comparable(
    env: &mut Env,
    position: &Span<'_>,
    comparable: &ast::Comparable<'_>,
) -> Result<CompareValue, CompileError> {
    Ok(match comparable {
        ast::Comparable::Int(value) => CompareValue::Value(Value::from(*value)),
        ast::Comparable::Float(value) => CompareValue::Value(Value::from(*value)),
        ast::Comparable::Variable(variable) => {
            CompareValue::Binding(existing_named_binding(env, variable, position)?)
        },
    })
}

fn compile_select_comparison(
    env: &mut Env,
    comparison: &ast::Comparison<'_>,
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    ops.push(CfgOpSelect::Compare {
        operator: comparison.ordering,
        left: compile_comparable(env, &comparison.position, &comparison.left)?,
        right: compile_comparable(env, &comparison.position, &comparison.right)?,
    });
    Ok(())
}

fn compile_select_toplevel_binding(
    env: &mut Env,
    variable: &ast::Variable<'_>,
    position: &Span<'_>,
    value_spec: &ast::ValueSpec<'_>,
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    let (binding, variable_name) = existing_named_binding_with_name(env, variable, position)?;
    match &value_spec.kind {
        ast::ValueSpecKind::Literal(literal) => {
            ops.push(CfgOpSelect::CompareBinding {
                binding,
                value: literal.to_value(),
            });
            Ok(())
        },
        ast::ValueSpecKind::Enum(ast::Bindable { variable: direct, inner: options }) => {
            no_binding(direct, position)?;
            compile_select_enum(env, binding, options, position, ops)
        },
        ast::ValueSpecKind::Tuple(ast::Bindable { variable: direct, inner: items }) => {
            no_binding(direct, position)?;
            compile_select_tuple(env, binding, items, ops)
        },
        ast::ValueSpecKind::Struct(ast::Bindable { variable: direct, inner: attributes }) => {
            no_binding(direct, position)?;
            ops.push(CfgOpSelect::AssertObjectBinding { binding });
            compile_select_attributes(env, binding, attributes, position, ops)
        },
        _ => Err(CompileError::IllegalBindingMatch {
            line: position.location_line(),
            name: variable_name,
        }),
    }
}

fn compile_select_tuple(
    env: &mut Env,
    binding: Binding,
    items: &[ast::ValueSpec],
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    let mut cfg_tuple_items = Vec::new();
    for ast::ValueSpec { position, kind } in items {
        match kind {
            ast::ValueSpecKind::Literal(literal) => {
                cfg_tuple_items.push(OpenTupleItem::Compare(literal.to_value()));
            },
            ast::ValueSpecKind::Variable(variable) => {
                match optional_binding(env, variable) {
                    Some(item_binding) => {
                        cfg_tuple_items.push(OpenTupleItem::Binding(item_binding));
                    },
                    None => {
                        cfg_tuple_items.push(OpenTupleItem::Ignore);
                    },
                }
            },
            ast::ValueSpecKind::Enum(ast::Bindable { variable: direct, inner: options }) => {
                let item_binding = nameable_binding(env, direct);
                compile_select_enum(env, item_binding, options, position, ops)?;
                cfg_tuple_items.push(OpenTupleItem::Binding(item_binding));
            },
            ast::ValueSpecKind::Tuple(ast::Bindable { variable: direct, inner: items }) => {
                let item_binding = nameable_binding(env, direct);
                compile_select_tuple(env, item_binding, items, ops)?;
                cfg_tuple_items.push(OpenTupleItem::Binding(item_binding));
            },
            ast::ValueSpecKind::Struct(ast::Bindable { variable: direct, inner: attributes }) => {
                let item_binding = nameable_binding(env, direct);
                ops.push(CfgOpSelect::AssertObjectBinding { binding: item_binding });
                compile_select_attributes(env, item_binding, attributes, position, ops)?;
                cfg_tuple_items.push(OpenTupleItem::Binding(item_binding));
            },
        }
    }
    ops.push(CfgOpSelect::TupleBinding {
        binding,
        values: cfg_tuple_items,
    });
    Ok(())
}

fn compile_select_attribute(
    env: &mut Env,
    binding: Binding,
    attribute: &ast::AttributeSpec<'_>,
    position: &Span<'_>,
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    let ast::AttributeSpec { attribute, value_spec, .. } = attribute;
    match &value_spec.kind {
        ast::ValueSpecKind::Literal(literal) => {
            ops.push(CfgOpSelect::RequireValueAttribute {
                binding,
                attribute: attribute.as_str().into(),
                value: literal.to_value(),
            });
            Ok(())
        },
        ast::ValueSpecKind::Variable(variable) => {
            match optional_binding(env, variable) {
                Some(value_binding) => {
                    ops.push(CfgOpSelect::AttributeBinding {
                        binding,
                        attribute: attribute.as_str().into(),
                        value_binding,
                    });
                },
                None => {
                    ops.push(CfgOpSelect::RequireAttribute {
                        binding,
                        attribute: attribute.as_str().into(),
                    });
                },
            }
            Ok(())
        },
        ast::ValueSpecKind::Tuple(ast::Bindable { variable: direct, inner: items }) => {
            let value_binding = nameable_binding(env, direct);
            ops.push(CfgOpSelect::AttributeBinding {
                binding,
                attribute: attribute.as_str().into(),
                value_binding,
            });
            compile_select_tuple(env, value_binding, items, ops)
        },
        ast::ValueSpecKind::Enum(ast::Bindable { variable: direct, inner: options }) => {
            let value_binding = nameable_binding(env, direct);
            ops.push(CfgOpSelect::AttributeBinding {
                binding,
                attribute: attribute.as_str().into(),
                value_binding,
            });
            compile_select_enum(env, value_binding, options, position, ops)
        },
        ast::ValueSpecKind::Struct(ast::Bindable { variable: direct, inner: attributes }) => {
            let value_binding = nameable_binding(env, direct);
            ops.push(CfgOpSelect::AttributeBinding {
                binding,
                attribute: attribute.as_str().into(),
                value_binding,
            });
            ops.push(CfgOpSelect::AssertObjectBinding { binding: value_binding });
            compile_select_attributes(env, value_binding, attributes, position, ops)
        },
    }
}

fn compile_select_attributes(
    env: &mut Env,
    binding: Binding,
    attributes: &[ast::AttributeSpec<'_>],
    position: &Span<'_>,
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    for attribute in attributes {
        compile_select_attribute(env, binding, attribute, position, ops)?;
    }
    Ok(())
}

fn compile_select_enum(
    env: &mut Env,
    binding: Binding,
    options: &[ast::Enumerable<'_>],
    position: &Span<'_>,
    ops: &mut Vec<CfgOpSelect>,
) -> Result<(), CompileError> {
    let mut cfg_enum_items = Vec::new();
    for option in options {
        match option {
            ast::Enumerable::Literal(literal) => {
                cfg_enum_items.push(EnumOption::Value(literal.to_value()));
            },
            ast::Enumerable::Variable(variable) => {
                let item_binding = existing_named_binding(env, variable, position)?;
                cfg_enum_items.push(EnumOption::Binding(item_binding));
            },
        }
    }
    ops.push(CfgOpSelect::EnumBinding {
        binding,
        options: cfg_enum_items,
    });
    Ok(())
}

fn no_binding(variable: &ast::Variable<'_>, position: &Span<'_>) -> Result<(), CompileError> {
    if let Some(name) = variable.as_str() {
        Err(CompileError::IllegalNamedBinding {
            line: position.location_line(),
            name: name.into(),
        })
    } else {
        Ok(())
    }
}

fn optional_binding(
    env: &mut Env<'_>,
    variable: &ast::Variable<'_>,
) -> Option<Binding> {
    if let Some(name) = variable.as_str() {
        Some(env.bind(name))
    } else {
        None
    }
}

fn nameable_binding(
    env: &mut Env<'_>,
    variable: &ast::Variable<'_>,
) -> Binding {
    match variable.as_str() {
        Some(name) => env.bind(name),
        None => env.anon(),
    }
}

fn named_new_binding(
    env: &mut Env<'_>,
    variable: &ast::Variable<'_>,
    position: &Span<'_>,
) -> Result<Binding, CompileError> {
    if let Some(name) = variable.as_str() {
        if let Some(_) = env.find(name) {
            Err(CompileError::IllegalReuse {
                line: position.location_line(),
                name: name.into(),
            })
        } else {
            Ok(env.bind(name))
        }
    } else {
        Err(CompileError::IllegalWildcard { line: position.location_line() })
    }
}

#[derive(Debug, Clone)]
pub enum CfgOpSelect {
    AssertObjectBinding {
        binding: Binding,
    },
    CompareBinding {
        binding: Binding,
        value: Value,
    },
    TupleBinding {
        binding: Binding,
        values: Vec<OpenTupleItem>,
    },
    EnumBinding {
        binding: Binding,
        options: Vec<EnumOption>,
    },
    RequireValueAttribute {
        binding: Binding,
        attribute: Symbol,
        value: Value,
    },
    AttributeBinding {
        binding: Binding,
        attribute: Symbol,
        value_binding: Binding,
    },
    RequireAttribute {
        binding: Binding,
        attribute: Symbol,
    },
    Not {
        body: Vec<CfgOpSelect>,
        inherited_bindings: FnvHashSet<Binding>,
    },
    Compare {
        operator: CompareOp,
        left: CompareValue,
        right: CompareValue,
    },
    Calculation {
        result_binding: Binding,
        operation: Calculation,
    },
}
