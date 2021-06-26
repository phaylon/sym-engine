
use std::cmp::{Ordering};
use num_traits::{ToPrimitive};
use crate::{Value, Access, Transaction, ValuesIter, RemovalMode};
use crate::data::{CompareOp, ArithBinOp};
use crate::compiler::{
    CompiledRule,
    Op,
    OpApply,
    TupleItem,
    ApplyTupleItem,
    EnumOption,
    Calculation,
};

pub fn splinter_rule<'space, F>(
    rule: &CompiledRule,
    tx: &Transaction<'space>,
    bindings: &mut [Value],
    mut collect: F,
) -> usize
where
    F: FnMut(Transaction<'space>) -> RuntimeControl,
{
    let mut count = 0;
    search_bindings(rule.ops(), tx, bindings, |bindings| {
        let mut new_tx = tx.clone();
        if apply_changes(rule.apply_ops(), &mut new_tx, bindings) {
            count += 1;

            #[cfg(feature = "tracing")]
            tracing::trace!(rule = rule.name().as_ref(), outcome = "produced-transaction");

            collect(new_tx)
        } else {

            #[cfg(feature = "tracing")]
            tracing::trace!(rule = rule.name().as_ref(), outcome = "failed application");

            RuntimeControl::Continue
        }
    });
    count
}

pub fn attempt_rule_firing(
    rule: &CompiledRule,
    space: &mut dyn Access,
    bindings: &mut [Value],
) -> bool {
    space.transaction(&mut |mut tx| {
        if find_first_bindings(rule.ops(), &mut tx, bindings) {
            if apply_changes(rule.apply_ops(), &mut tx, bindings) {

                #[cfg(feature = "tracing")]
                tracing::trace!(rule = rule.name().as_ref(), outcome = "applied");

                Some(tx)
            } else {

                #[cfg(feature = "tracing")]
                tracing::trace!(rule = rule.name().as_ref(), outcome = "failed application");

                None
            }
        } else {

            #[cfg(feature = "tracing")]
            tracing::trace!(rule = rule.name().as_ref(), outcome = "failed match");

            None
        }
    })
}

fn apply_changes(
    apply_ops: &[OpApply],
    space: &mut dyn Access,
    bindings: &mut [Value],
) -> bool {
    for op in apply_ops {
        match op {
            OpApply::CreateObject { binding } => {
                let id = space.create_id();
                bindings[binding.index()] = Value::Object(id);
            },
            OpApply::CreateTuple { binding, items } => {
                let values = items
                    .iter()
                    .map(|item| match item {
                        ApplyTupleItem::Value(value) => value.clone(),
                        ApplyTupleItem::Binding(binding) => bindings[binding.index()].clone(),
                    })
                    .collect();
                bindings[binding.index()] = Value::Tuple(values);
            },
            OpApply::AddBindingAttribute { binding, attribute, value_binding } => {
                if let Some(id) = bindings[binding.index()].object() {
                    space.attributes_mut(id)
                        .add(attribute.clone(), bindings[value_binding.index()].clone());
                } else {
                    return false;
                }
            },
            OpApply::RemoveBindingAttribute { binding, attribute, value_binding, mode } => {
                if let Some(id) = bindings[binding.index()].object() {
                    let removed = space.attributes_mut(id)
                        .remove_single(attribute, &bindings[value_binding.index()]);
                    if removed.is_none() {
                        if let RemovalMode::Required = mode {
                            return false;
                        }
                    }
                } else {
                    return false;
                }
            },
            OpApply::AddValueAttribute { binding, attribute, value } => {
                if let Some(id) = bindings[binding.index()].object() {
                    space.attributes_mut(id)
                        .add(attribute.clone(), value.clone());
                } else {
                    return false;
                }
            },
            OpApply::RemoveValueAttribute { binding, attribute, value, mode } => {
                if let Some(id) = bindings[binding.index()].object() {
                    let removed = space.attributes_mut(id)
                        .remove_single(attribute, value);
                    if removed.is_none() {
                        if let RemovalMode::Required = mode {
                            return false;
                        }
                    }
                } else {
                    return false;
                }
            },
            OpApply::Conditional { condition, then_apply, otherwise_apply } => {
                let mut local_bindings = bindings.to_vec();
                let continue_apply =
                    if find_first_bindings(condition, space, &mut local_bindings) {
                        apply_changes(then_apply, space, bindings)
                    } else {
                        apply_changes(otherwise_apply, space, bindings)
                    };
                if !continue_apply {
                    return false;
                }
            },
        }
    }
    true
}

pub fn find_first_bindings(
    ops: &[Op],
    space: &dyn Access,
    bindings: &mut [Value],
) -> bool {
    search_bindings(ops, space, bindings, |_| RuntimeControl::Stop)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeControl {
    Continue,
    Stop,
}

fn search_bindings<F>(
    ops: &[Op],
    space: &dyn Access,
    bindings: &mut [Value],
    mut control: F,
) -> bool
where
    F: FnMut(&mut [Value]) -> RuntimeControl,
{
    let mut op_index = 0;
    let mut frames = Vec::new();

    loop {
        let flow = match &ops[op_index] {
            Op::AssertObjectBinding { binding } => {
                if bindings[binding.index()].object().is_some() {
                    Flow::NextOp
                } else {
                    Flow::NextBranch
                }
            },
            Op::RequireAttributeBinding { binding, attribute, value_binding } => {
                if let Some(id) = bindings[binding.index()].object() {
                    if space.attributes(id)
                        .has(attribute.as_ref(), &bindings[value_binding.index()])
                    {
                        Flow::NextOp
                    } else {
                        Flow::NextBranch
                    }
                } else {
                    Flow::NextBranch
                }
            },
            Op::RequireAttributeValue { binding, attribute, value } => {
                if let Some(id) = bindings[binding.index()].object() {
                    if space.attributes(id).has(attribute.as_ref(), value) {
                        Flow::NextOp
                    } else {
                        Flow::NextBranch
                    }
                } else {
                    Flow::NextBranch
                }
            },
            Op::RequireAttribute { binding, attribute } => {
                if let Some(id) = bindings[binding.index()].object() {
                    if space.attributes(id).has_named(attribute.as_ref()) {
                        Flow::NextOp
                    } else {
                        Flow::NextBranch
                    }
                } else {
                    Flow::NextBranch
                }
            },
            Op::CompareBinding { binding, value } => {
                if &bindings[binding.index()] == value {
                    Flow::NextOp
                } else {
                    Flow::NextBranch
                }
            },
            Op::SearchAttributeBinding { binding, attribute, value_binding } => {
                if let Some(id) = bindings[binding.index()].object() {
                    let iter = space.attributes(id).iter_named(attribute);
                    frames.push(Frame::Iter {
                        binding: value_binding.index(),
                        continue_op_index: op_index + 1,
                        iter,
                    });
                    Flow::NextBranch
                } else {
                    Flow::NextBranch
                }
            }
            Op::UnpackTupleBinding { binding, values } => {
                if let Some(tuple) = bindings[binding.index()].tuple().cloned() {
                    if tuple.len() == values.len() {
                        let matched = tuple.iter().zip(values.iter())
                            .all(|(value, expected)| {
                                match expected {
                                    TupleItem::Ignore => true,
                                    TupleItem::Bind(binding) => {
                                        bindings[binding.index()] = value.clone();
                                        true
                                    },
                                    TupleItem::CompareBinding(binding) => {
                                        bindings[binding.index()] == *value
                                    },
                                    TupleItem::CompareValue(expected_value) => {
                                        expected_value == value
                                    },
                                }
                            });
                        if matched {
                            Flow::NextOp
                        } else {
                            Flow::NextBranch
                        }
                    } else {
                        Flow::NextBranch
                    }
                } else {
                    Flow::NextBranch
                }
            },
            Op::MatchEnumBinding { binding, options } => {
                let mut matched = false;
                'options: for option in options {
                    match option {
                        EnumOption::Binding(match_binding) => {
                            if bindings[binding.index()] == bindings[match_binding.index()] {
                                matched = true;
                                break 'options;
                            }
                        },
                        EnumOption::Value(value) => {
                            if bindings[binding.index()] == *value {
                                matched = true;
                                break 'options;
                            }
                        },
                    }
                }
                if matched {
                    Flow::NextOp
                } else {
                    Flow::NextBranch
                }
            },
            Op::Compare { comparison } => {
                let left_value = comparison.left.resolve(bindings);
                let right_value = comparison.right.resolve(bindings);
                if let Some((left_value, right_value))
                    = unify_numeric_types(left_value.clone(), right_value.clone())
                {
                    let cmp = left_value.partial_cmp(&right_value);
                    let matched = match cmp {
                        Some(Ordering::Equal) => match comparison.operator {
                            CompareOp::Equal | CompareOp::LessOrEqual | CompareOp::GreaterOrEqual
                                => true,
                            _ => false,
                        },
                        Some(Ordering::Less) => match comparison.operator {
                            CompareOp::Less | CompareOp::LessOrEqual | CompareOp::NotEqual
                                => true,
                            _ => false,
                        },
                        Some(Ordering::Greater) => match comparison.operator {
                            CompareOp::Greater | CompareOp::GreaterOrEqual | CompareOp::NotEqual
                                => true,
                            _ => false,
                        },
                        None => match comparison.operator {
                            CompareOp::NotEqual => true,
                            _ => false,
                        },
                    };
                    if matched {
                        Flow::NextOp
                    } else {
                        Flow::NextBranch
                    }
                } else {
                    Flow::NextBranch
                }
            },
            Op::Calculation { binding, operation } => {
                match perform_calculation(bindings, operation) {
                    Some(value) => {
                        bindings[binding.index()] = value;
                        Flow::NextOp
                    },
                    None => {
                        Flow::NextBranch
                    },
                }
            },
            Op::BeginNot { index, sequence_len } => {
                frames.push(Frame::NotScope {
                    index: *index,
                    continue_ok: op_index + *sequence_len + 1,
                });
                Flow::NextOp
            },
            Op::EndNot { index } => {
                let frame_index = frames
                    .iter()
                    .position(|frame| match frame {
                        Frame::NotScope { index: fr_index, .. } => *fr_index == *index,
                        _ => false,
                    })
                    .expect("corresponding not-scope frame");
                frames.truncate(frame_index);
                Flow::NextBranch
            },
            Op::End => {
                match control(bindings) {
                    RuntimeControl::Continue => Flow::NextBranch,
                    RuntimeControl::Stop => {
                        return true;
                    },
                }
            },
        };
        match flow {
            Flow::NextOp => {
                op_index += 1;
            },
            Flow::NextBranch => {
                'next_branch: loop {
                    if let Some(frame) = frames.last_mut() {
                        match frame {
                            Frame::NotScope { continue_ok, .. } => {
                                op_index = *continue_ok;
                                frames.pop();
                            },
                            Frame::Iter { iter, binding, continue_op_index } => {
                                if let Some(value) = iter.next() {
                                    bindings[*binding] = value.clone();
                                    op_index = *continue_op_index;
                                } else {
                                    frames.pop();
                                    continue 'next_branch;
                                }
                            },
                        }
                    } else {
                        return false;
                    }
                    break 'next_branch;
                }
            },
        }
    }
}

fn perform_calculation(bindings: &[Value], calc: &Calculation) -> Option<Value> {
    match calc {
        Calculation::Value(value) => Some(value.clone()),
        Calculation::Binding(binding) => Some(bindings[binding.index()].clone()),
        Calculation::BinOp(op, left, right) => {
            let (left, right) = unify_numeric_types(
                perform_calculation(bindings, left)?,
                perform_calculation(bindings, right)?,
            )?;
            match op {
                ArithBinOp::Add => match (left, right) {
                    (Value::Int(left), Value::Int(right))
                        => Some(Value::Int(left.checked_add(right)?)),
                    (Value::Float(left), Value::Float(right))
                        => Some(Value::Float(left + right)),
                    _ => None,
                },
                ArithBinOp::Sub => match (left, right) {
                    (Value::Int(left), Value::Int(right))
                        => Some(Value::Int(left.checked_sub(right)?)),
                    (Value::Float(left), Value::Float(right))
                        => Some(Value::Float(left - right)),
                    _ => None,
                },
                ArithBinOp::Mul => match (left, right) {
                    (Value::Int(left), Value::Int(right))
                        => Some(Value::Int(left.checked_mul(right)?)),
                    (Value::Float(left), Value::Float(right))
                        => Some(Value::Float(left * right)),
                    _ => None,
                },
                ArithBinOp::Div => match (left, right) {
                    (Value::Int(left), Value::Int(right))
                        => Some(Value::Int(left.checked_div(right)?)),
                    (Value::Float(left), Value::Float(right))
                        => if right != 0.0 {
                            Some(Value::Float(left / right))
                        } else {
                            None
                        }
                    _ => None,
                },
            }
        },
    }
}

fn unify_numeric_types(left: Value, right: Value) -> Option<(Value, Value)> {
    match (left, right) {
        (Value::Int(left_val), right @ Value::Float(_)) =>
            Some((Value::Float(left_val.to_f64()?), right)),
        (left @ Value::Float(_), Value::Int(right_val)) =>
            Some((left, Value::Float(right_val.to_f64()?))),
        (left, right) => Some((left, right)),
    }
}

enum Frame<'a> {
    Iter {
        iter: ValuesIter<'a>,
        binding: usize,
        continue_op_index: usize,
    },
    NotScope {
        index: usize,
        continue_ok: usize,
    },
}

enum Flow {
    NextBranch,
    NextOp,
}