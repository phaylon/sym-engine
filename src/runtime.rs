
use std::cmp::{Ordering};
use num_traits::{ToPrimitive};
use crate::{Value, Access, Symbol, AttributesIter};
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

pub fn attempt_rule_firing(
    rule: &CompiledRule,
    space: &mut dyn Access,
    bindings: &mut [Value],
) -> bool {
    space.transaction(&mut |mut tx| {
        if find_bindings(rule.ops(), &mut tx, bindings) {
            if apply_changes(rule.apply_ops(), &mut tx, bindings) {
                Some(tx)
            } else {
                None
            }
        } else {
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
                bindings[*binding] = Value::Object(id);
            },
            OpApply::CreateTuple { binding, items } => {
                let values = items
                    .iter()
                    .map(|item| match item {
                        ApplyTupleItem::Value(value) => value.clone(),
                        ApplyTupleItem::Binding(binding) => bindings[*binding].clone(),
                    })
                    .collect();
                bindings[*binding] = Value::Tuple(values);
            },
            OpApply::AddBindingAttribute { binding, attribute, value_binding } => {
                if let Some(id) = bindings[*binding].object() {
                    space.attributes_mut(id)
                        .add(attribute.clone(), bindings[*value_binding].clone());
                } else {
                    return false;
                }
            },
            OpApply::RemoveBindingAttribute { binding, attribute, value_binding } => {
                if let Some(id) = bindings[*binding].object() {
                    let removed = space.attributes_mut(id)
                        .remove_first(attribute, &bindings[*value_binding]);
                    if removed.is_none() {
                        return false;
                    }
                } else {
                    return false;
                }
            },
            OpApply::AddValueAttribute { binding, attribute, value } => {
                if let Some(id) = bindings[*binding].object() {
                    space.attributes_mut(id)
                        .add(attribute.clone(), value.clone());
                } else {
                    return false;
                }
            },
            OpApply::RemoveValueAttribute { binding, attribute, value } => {
                if let Some(id) = bindings[*binding].object() {
                    let removed = space.attributes_mut(id)
                        .remove_first(attribute, value);
                    if removed.is_none() {
                        return false;
                    }
                } else {
                    return false;
                }
            },
        }
    }
    true
}

fn find_bindings(
    ops: &[Op],
    space: &mut dyn Access,
    bindings: &mut [Value],
) -> bool {

    let mut op_index = 0;
    let mut frames = Vec::new();

    loop {
        let flow = match &ops[op_index] {
            Op::AssertObjectBinding { binding } => {
                if bindings[*binding].object().is_some() {
                    Flow::NextOp
                } else {
                    Flow::NextBranch
                }
            },
            Op::RequireAttributeBinding { binding, attribute, value_binding } => {
                if let Some(id) = bindings[*binding].object() {
                    if space.attributes(id).has(attribute.as_ref(), &bindings[*value_binding]) {
                        Flow::NextOp
                    } else {
                        Flow::NextBranch
                    }
                } else {
                    Flow::NextBranch
                }
            },
            Op::RequireAttributeValue { binding, attribute, value } => {
                if let Some(id) = bindings[*binding].object() {
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
                if let Some(id) = bindings[*binding].object() {
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
                if &bindings[*binding] == value {
                    Flow::NextOp
                } else {
                    Flow::NextBranch
                }
            },
            Op::SearchAttributeBinding { binding, attribute, value_binding } => {
                if let Some(id) = bindings[*binding].object() {
                    let iter = space.attributes(id).iter();
                    frames.push(Frame::Iter {
                        attribute: attribute.clone(),
                        binding: *value_binding,
                        continue_op_index: op_index + 1,
                        iter,
                    });
                    Flow::NextBranch
                } else {
                    Flow::NextBranch
                }
            }
            Op::UnpackTupleBinding { binding, values } => {
                if let Some(tuple) = bindings[*binding].tuple().cloned() {
                    if tuple.len() == values.len() {
                        let matched = tuple.iter().zip(values.iter())
                            .all(|(value, expected)| {
                                match expected {
                                    TupleItem::Ignore => true,
                                    TupleItem::Bind(binding) => {
                                        bindings[*binding] = value.clone();
                                        true
                                    },
                                    TupleItem::CompareBinding(binding) => {
                                        bindings[*binding] == *value
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
                            if bindings[*binding] == bindings[*match_binding] {
                                matched = true;
                                break 'options;
                            }
                        },
                        EnumOption::Value(value) => {
                            if bindings[*binding] == *value {
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
            Op::Compare { operator, left, right } => {
                let left_value = left.resolve(bindings);
                let right_value = right.resolve(bindings);
                if let Some((left_value, right_value))
                    = unify_numeric_types(left_value.clone(), right_value.clone())
                {
                    let cmp = left_value.partial_cmp(&right_value);
                    let matched = match cmp {
                        Some(Ordering::Equal) => match operator {
                            CompareOp::Equal | CompareOp::LessOrEqual | CompareOp::GreaterOrEqual
                                => true,
                            _ => false,
                        },
                        Some(Ordering::Less) => match operator {
                            CompareOp::Less | CompareOp::LessOrEqual | CompareOp::NotEqual
                                => true,
                            _ => false,
                        },
                        Some(Ordering::Greater) => match operator {
                            CompareOp::Greater | CompareOp::GreaterOrEqual | CompareOp::NotEqual
                                => true,
                            _ => false,
                        },
                        None => match operator {
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
                        bindings[*binding] = value;
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
                    continue_ok: op_index + *sequence_len,
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
                return true;
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
                            },
                            Frame::Iter { attribute, iter, binding, continue_op_index } => {
                                let mut found = None;
                                'attributes: for (name, value) in iter {
                                    if name == attribute {
                                        found = Some(value.clone());
                                        break 'attributes;
                                    }
                                }
                                if let Some(value) = found {
                                    bindings[*binding] = value;
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
        Calculation::Binding(binding) => Some(bindings[*binding].clone()),
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
        attribute: Symbol,
        iter: AttributesIter<'a>,
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