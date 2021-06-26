
use fnv::{FnvHashSet};
use float_ord::{FloatOrd};
use super::cfg::{CfgRule};
use super::cfg_ops::{CfgOpSelect, CfgOpApply, OpenTupleItem};
use super::ops::{Op, OpApply};
use super::{ops, EnumOption, Binding};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct JumpIndex(usize);

fn eliminate_object_assertions(select: &mut Vec<CfgOpSelect>) {

    let mut known_object_bindings = FnvHashSet::default();
    for op in select.iter() {
        let binding = match *op {
            CfgOpSelect::AttributeBinding { binding, .. } |
            CfgOpSelect::RequireAttribute { binding, .. } |
            CfgOpSelect::RequireValueAttribute { binding, .. } => binding,
            _ => {
                continue;
            },
        };
        known_object_bindings.insert(binding);
    }
    let mut asserted_objected_bindings = FnvHashSet::default();
    select.retain(|op| match *op {
        CfgOpSelect::AssertObjectBinding { binding } => {
            if known_object_bindings.contains(&binding) {
                false
            } else {
                if asserted_objected_bindings.contains(&binding) {
                    false
                } else {
                    asserted_objected_bindings.insert(binding);
                    true
                }
            }
        },
        _ => true,
    });

    for op in select.iter_mut() {
        if let CfgOpSelect::Not { body, .. } = op {
            eliminate_object_assertions(body);
        }
    }
}

struct Sequence {
    index: usize,
}

impl Sequence {

    fn new() -> Self {
        Self {
            index: 0,
        }
    }

    fn next(&mut self) -> usize {
        let index = self.index;
        self.index += 1;
        index
    }
}

pub fn optimize(rule: &CfgRule, input_bindings_len: usize) -> (Vec<Op>, Vec<OpApply>) {

    let mut sequence = Sequence::new();
    let provided = (0..input_bindings_len)
        .map(Binding::with_index)
        .collect::<Vec<Binding>>();

    let (st_provided, select_ops) = optimize_select(&mut sequence, &rule.select, &provided);
    let apply_ops = optimize_apply(&mut sequence, &rule.apply, &st_provided);

    (select_ops, apply_ops)
}

fn optimize_apply(
    sequence: &mut Sequence,
    cfg_ops: &[CfgOpApply],
    provided: &[Binding],
) -> Vec<OpApply> {

    let mut ops = Vec::new();
    for cfg_op in cfg_ops {
        ops.push(match *cfg_op {
            CfgOpApply::CreateObject { binding } =>
                OpApply::CreateObject { binding },
            CfgOpApply::CreateTuple { binding, ref items } =>
                OpApply::CreateTuple { binding, items: items.clone() },
            CfgOpApply::AddBindingAttribute { binding, ref attribute, value_binding } =>
                OpApply::AddBindingAttribute {
                    binding,
                    attribute: attribute.clone(),
                    value_binding,
                },
            CfgOpApply::RemoveBindingAttribute { binding, ref attribute, value_binding, mode } =>
                OpApply::RemoveBindingAttribute {
                    binding,
                    attribute: attribute.clone(),
                    value_binding,
                    mode,
                },
            CfgOpApply::AddValueAttribute { binding, ref attribute, ref value } =>
                OpApply::AddValueAttribute {
                    binding,
                    attribute: attribute.clone(),
                    value: value.clone(),
                },
            CfgOpApply::RemoveValueAttribute { binding, ref attribute, ref value, mode } =>
                OpApply::RemoveValueAttribute {
                    binding,
                    attribute: attribute.clone(),
                    value: value.clone(),
                    mode,
                },
            CfgOpApply::Conditional { ref condition, ref then_apply, ref otherwise_apply } => {
                let (_, condition) = optimize_select(sequence, condition, provided);
                let then_apply = optimize_apply(sequence, then_apply, provided);
                let otherwise_apply = optimize_apply(sequence, otherwise_apply, provided);
                OpApply::Conditional { condition, then_apply, otherwise_apply }
            },
        });
    }
    ops
}

fn optimize_select(
    sequence: &mut Sequence,
    cfg_ops: &[CfgOpSelect],
    provided: &[Binding],
) -> (Vec<Binding>, Vec<Op>) {

    let mut cfg_ops = cfg_ops.to_vec();
    eliminate_object_assertions(&mut cfg_ops);

    let mut state = assemble_ops(&cfg_ops, &OpState::new(provided), sequence)
        .expect("select op order solution");
    state.ops.push(Op::End);

    (state.provided, state.ops)
}

fn assemble_ops(select: &[CfgOpSelect], prev: &OpState, seq: &mut Sequence) -> Option<OpState> {
    let mut branches = Vec::new();
    branches.push((prev.clone(), select.to_vec()));

    let mut branches_next = Vec::new();
    for _ in 0..select.len() {
        if branches.is_empty() {
            return None;
        }
        for (branch, rest_ops) in branches.drain(..) {
            for next_op_index in 0..rest_ops.len() {
                if let Some(next_state) = transform_op(&rest_ops[next_op_index], &branch, seq) {
                    branches_next.push((
                        next_state,
                        rest_ops[0..next_op_index]
                            .iter()
                            .chain(rest_ops[(next_op_index + 1)..].iter())
                            .cloned()
                            .collect(),
                    ));
                }
            }
        }
        branches_next.sort_by(|a, b| {
            FloatOrd(a.0.cost).cmp(&FloatOrd(b.0.cost))
        });
        branches_next.truncate(16);
        std::mem::swap(&mut branches, &mut branches_next);
    }

    if !branches.is_empty() {
        let (selected_state, rest_ops) = branches.remove(0);
        assert!(rest_ops.is_empty(), "all cfg ops consumed");
        Some(selected_state)
    } else {
        None
    }
}

fn transform_op(op: &CfgOpSelect, prev: &OpState, seq: &mut Sequence) -> Option<OpState> {
    use std::iter::{empty, once};

    match op {
        CfgOpSelect::AssertObjectBinding { binding } => {
            prev.bound(*binding).then(|| {
                prev.advance(
                    Op::AssertObjectBinding { binding: *binding },
                    |cost| cost - 1.0,
                    empty(),
                )
            })
        },
        CfgOpSelect::CompareBinding { binding, value } => {
            prev.bound(*binding).then(|| {
                prev.advance(
                    Op::CompareBinding { binding: *binding, value: value.clone() },
                    |cost| cost - 1.2,
                    empty(),
                )
            })
        },
        CfgOpSelect::TupleBinding { binding, values } => {
            prev.bound(*binding).then(|| {
                let mut new_bindings = Vec::new();
                let mut new_values = Vec::new();
                for value in values {
                    match value {
                        OpenTupleItem::Ignore => {
                            new_values.push(ops::TupleItem::Ignore);
                        },
                        OpenTupleItem::Compare(value) => {
                            new_values.push(ops::TupleItem::CompareValue(value.clone()));
                        },
                        OpenTupleItem::Binding(binding) => {
                            if prev.bound(*binding) || new_bindings.contains(binding) {
                                new_values.push(ops::TupleItem::CompareBinding(*binding));
                            } else {
                                new_bindings.push(*binding);
                                new_values.push(ops::TupleItem::Bind(*binding));
                            }
                        },
                    }
                }
                let no_new_bindings = new_bindings.is_empty();
                prev.advance(
                    Op::UnpackTupleBinding {
                        binding: *binding,
                        values: new_values,
                    },
                    |cost| {
                        if no_new_bindings {
                            cost - 1.2
                        } else {
                            cost - 1.0
                        }
                    },
                    new_bindings.into_iter(),
                )
            })
        },
        CfgOpSelect::EnumBinding { binding, options } => {
            if (
                prev.bound(*binding)
                && prev.all_bound(options.iter().filter_map(EnumOption::to_binding))
            ) {
                Some(prev.advance(
                    Op::MatchEnumBinding {
                        binding: *binding,
                        options: options.clone(),
                    },
                    |cost| cost - 1.2,
                    empty(),
                ))
            } else {
                None
            }
        },
        CfgOpSelect::RequireValueAttribute { binding, attribute, value } => {
            prev.bound(*binding).then(|| {
                prev.advance(
                    Op::RequireAttributeValue {
                        binding: *binding,
                        attribute: attribute.clone(),
                        value: value.clone(),
                    },
                    |cost| cost - 1.3,
                    empty(),
                )
            })
        },
        CfgOpSelect::RequireAttribute { binding, attribute } => {
            prev.bound(*binding).then(|| {
                prev.advance(
                    Op::RequireAttribute {
                        binding: *binding,
                        attribute: attribute.clone(),
                    },
                    |cost| cost - 2.0,
                    empty(),
                )
            })
        },
        CfgOpSelect::AttributeBinding { binding, attribute, value_binding } => {
            prev.bound(*binding).then(|| {
                if prev.bound(*value_binding) {
                    prev.advance(
                        Op::RequireAttributeBinding {
                            binding: *binding,
                            attribute: attribute.clone(),
                            value_binding: *value_binding,
                        },
                        |cost| cost - 1.2,
                        empty(),
                    )
                } else {
                    prev.advance(
                        Op::SearchAttributeBinding {
                            binding: *binding,
                            attribute: attribute.clone(),
                            value_binding: *value_binding,
                        },
                        |cost| cost * 1.4,
                        once(*value_binding),
                    )
                }
            })
        },
        CfgOpSelect::Compare { operator, left, right } => {
            if (
                left.to_binding().map(|binding| prev.bound(binding)).unwrap_or(true)
                && right.to_binding().map(|binding| prev.bound(binding)).unwrap_or(true)
            ) {
                Some(prev.advance(
                    Op::Compare {
                        comparison: Box::new(ops::Comparison {
                            operator: *operator,
                            left: left.clone(),
                            right: right.clone(),
                        }),
                    },
                    |cost| cost - 2.0,
                    empty(),
                ))
            } else {
                None
            }
        },
        CfgOpSelect::Calculation { result_binding, operation } => {
            let bindings = operation.bindings();
            prev.all_bound(bindings.iter().copied()).then(|| {
                prev.advance(
                    Op::Calculation {
                        binding: *result_binding,
                        operation: operation.clone(),
                    },
                    |cost| cost,
                    once(*result_binding),
                )
            })
        }
        CfgOpSelect::Not { body, .. } => {
            if let Some(mut body_state) = assemble_ops(&body, prev, seq) {
                let index = seq.next();
                body_state.ops.push(Op::EndNot { index });
                let sequence_len = body_state.ops.len() - prev.ops.len();
                body_state.ops.insert(prev.ops.len(), Op::BeginNot { index, sequence_len });
                Some(body_state)
            } else {
                None
            }
        },
    }
}

#[derive(Debug, Clone)]
struct OpState {
    ops: Vec<Op>,
    cost: f64,
    provided: Vec<Binding>,
}

impl OpState {

    fn new(provided: &[Binding]) -> Self {
        Self {
            ops: Vec::new(),
            cost: 1000.0,
            provided: provided.into(),
        }
    }

    fn advance<I, C>(&self, op: Op, adjust_cost: C, new_bindings: I) -> Self
    where
        I: Iterator<Item = Binding>,
        C: FnOnce(f64) -> f64,
    {
        let mut ops = self.ops.clone();
        ops.push(op);

        let mut provided = self.provided.clone();
        for binding in new_bindings {
            if !provided.contains(&binding) {
                provided.push(binding);
            }
        }

        let cost = adjust_cost(self.cost);

        Self { ops, provided, cost }
    }

    fn bound(&self, binding: Binding) -> bool {
        self.provided.contains(&binding)
    }

    fn all_bound<I>(&self, mut bindings: I) -> bool
    where
        I: Iterator<Item = Binding>,
    {
        bindings.all(|binding| self.provided.contains(&binding))
    }
}
