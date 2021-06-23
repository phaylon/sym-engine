
use crate::{Symbol, Value};
use crate::data::{CompareOp, ArithBinOp};
use super::cfg::{CfgOpSelect};
use super::{
    BindingSequence,
    Binding,
    OpenTupleItem,
    EnumOption,
    CompareValue,
    Calculation,
    OpApply,
    ApplyTupleItem,
};

#[derive(Debug)]
pub struct BuiltRule {
    pub select: Vec<CfgOpSelect>,
    pub apply: Vec<OpApply>,
    pub bindings_len: usize,
}

pub fn build<F>(input_bindings_len: usize, builder_cb: F) -> BuiltRule
where
    F: for<'seq, 'bind> FnOnce(
        SelectBuilder<'seq, 'bind>,
        &[BuilderBinding<'bind>],
    ) -> ApplyBuilder<'seq, 'bind>,
{
    let binding_sequence = BindingSequence::new();
    let linked_binding_sequence = LinkedBindingSequence {
        binding_sequence: &binding_sequence,
        _bindings_lifetime: std::marker::PhantomData,
    };
    let input_bindings = (0..input_bindings_len)
        .into_iter()
        .map(|_| linked_binding_sequence.next())
        .collect::<Vec<_>>();
    let select_builder = SelectBuilder {
        binding_sequence: linked_binding_sequence,
        select: Vec::new(),
    };
    let ApplyBuilder { apply, select, .. } = builder_cb(select_builder, &input_bindings);
    let SelectBuilder { select, .. } = select;

    BuiltRule {
        select,
        apply,
        bindings_len: binding_sequence.len(),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LinkedBindingSequence<'seq, 'bind> {
    binding_sequence: &'seq BindingSequence,
    _bindings_lifetime: std::marker::PhantomData<fn() -> &'bind ()>,
}

impl<'seq, 'bind> LinkedBindingSequence<'seq, 'bind> {

    fn next(&self) -> BuilderBinding<'bind> {
        let inner = self.binding_sequence.next();
        BuilderBinding {
            inner,
            _bindings_lifetime: std::marker::PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuilderBinding<'bind> {
    inner: Binding,
    _bindings_lifetime: std::marker::PhantomData<fn() -> &'bind ()>,
}

#[derive(Debug)]
pub struct ApplyBuilder<'seq, 'bind> {
    binding_sequence: LinkedBindingSequence<'seq, 'bind>,
    select: SelectBuilder<'seq, 'bind>,
    apply: Vec<OpApply>,
}

impl<'seq, 'bind> ApplyBuilder<'seq, 'bind> {

    pub fn add_object_creation(&mut self) -> BuilderBinding<'bind> {
        let binding = self.binding_sequence.next();
        self.apply.push(OpApply::CreateObject {
            binding: binding.inner,
        });
        binding
    }

    pub fn add_tuple_creation<F>(&mut self, tuple_items_cb: F) -> BuilderBinding<'bind>
    where
        F: FnOnce(&mut ApplyTupleBuilder),
    {
        let binding = self.binding_sequence.next();
        let mut apply_tuple_builder = ApplyTupleBuilder {
            tuple_items: Vec::new(),
        };
        tuple_items_cb(&mut apply_tuple_builder);
        self.apply.push(OpApply::CreateTuple {
            binding: binding.inner,
            items: apply_tuple_builder.tuple_items,
        });
        binding
    }

    pub fn add_binding_attribute_addition<K>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
        value_binding: BuilderBinding<'bind>,
    )
    where
        K: Into<Symbol>,
    {
        self.apply.push(OpApply::AddBindingAttribute {
            binding: binding.inner,
            attribute: attribute.into(),
            value_binding: value_binding.inner,
        });
    }

    pub fn add_binding_attribute_removal<K>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
        value_binding: BuilderBinding<'bind>,
    )
    where
        K: Into<Symbol>,
    {
        self.apply.push(OpApply::RemoveBindingAttribute {
            binding: binding.inner,
            attribute: attribute.into(),
            value_binding: value_binding.inner,
        });
    }

    pub fn add_value_attribute_addition<K, V>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
        value: V,
    )
    where
        K: Into<Symbol>,
        V: Into<Value>,
    {
        self.apply.push(OpApply::AddValueAttribute {
            binding: binding.inner,
            attribute: attribute.into(),
            value: value.into(),
        });
    }

    pub fn add_value_attribute_removal<K, V>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
        value: V,
    )
    where
        K: Into<Symbol>,
        V: Into<Value>,
    {
        self.apply.push(OpApply::RemoveValueAttribute {
            binding: binding.inner,
            attribute: attribute.into(),
            value: value.into(),
        });
    }
}

#[derive(Debug)]
pub struct ApplyTupleBuilder {
    tuple_items: Vec<ApplyTupleItem>,
}

impl ApplyTupleBuilder {

    pub fn add_value_item<V>(&mut self, value: V)
    where
        V: Into<Value>,
    {
        self.tuple_items.push(ApplyTupleItem::Value(value.into()));
    }

    pub fn add_binding_item(&mut self, binding: BuilderBinding<'_>) {
        self.tuple_items.push(ApplyTupleItem::Binding(binding.inner));
    }
}

#[derive(Debug)]
pub struct SelectBuilder<'seq, 'bind> {
    binding_sequence: LinkedBindingSequence<'seq, 'bind>,
    select: Vec<CfgOpSelect>,
}

impl<'seq, 'bind> SelectBuilder<'seq, 'bind> {

    pub fn into_apply_builder<'select>(self) -> ApplyBuilder<'seq, 'bind> {
        let binding_sequence = self.binding_sequence;
        ApplyBuilder {
            binding_sequence,
            select: self,
            apply: Vec::new(),
        }
    }

    pub fn add_object_binding_assertion(
        &mut self,
        binding: BuilderBinding<'bind>,
    ) {
        self.select.push(CfgOpSelect::AssertObjectBinding {
            binding: binding.inner,
        });
    }

    pub fn add_binding_value_comparison<V>(
        &mut self,
        binding: BuilderBinding<'bind>,
        value: V,
    )
    where
        V: Into<Value>,
    {
        self.select.push(CfgOpSelect::CompareBinding {
            binding: binding.inner,
            value: value.into(),
        });
    }

    pub fn add_tuple_unpacking<R, F>(
        &mut self,
        binding: BuilderBinding<'bind>,
        tuple_builder_cb: F,
    ) -> R
    where
        F: FnOnce(&mut TupleBuilder<'seq, 'bind>) -> R,
    {
        let Self { binding_sequence, select } = self;
        let mut tuple_builder = TupleBuilder {
            binding_sequence: *binding_sequence,
            tuple_items: Vec::new(),
        };
        let result = tuple_builder_cb(&mut tuple_builder);
        select.push(CfgOpSelect::TupleBinding {
            binding: binding.inner,
            values: tuple_builder.tuple_items,
        });
        result
    }

    pub fn add_enum_match<F>(
        &mut self,
        binding: BuilderBinding<'bind>,
        enum_builder_cb: F,
    )
    where
        F: FnOnce(&mut EnumBuilder<'seq, 'bind>),
    {
        let Self { binding_sequence, select } = self;
        let mut enum_builder = EnumBuilder {
            binding_sequence: *binding_sequence,
            enum_options: Vec::new(),
        };
        enum_builder_cb(&mut enum_builder);
        select.push(CfgOpSelect::EnumBinding {
            binding: binding.inner,
            options: enum_builder.enum_options,
        });
    }

    pub fn add_attribute_value_requirement<K, V>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
        value: V,
    )
    where
        K: Into<Symbol>,
        V: Into<Value>,
    {
        self.select.push(CfgOpSelect::RequireValueAttribute {
            binding: binding.inner,
            attribute: attribute.into(),
            value: value.into(),
        });
    }

    pub fn add_attribute_binding<K>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
    ) -> BuilderBinding<'bind>
    where
        K: Into<Symbol>,
    {
        let value_binding = self.binding_sequence.next();
        self.select.push(CfgOpSelect::AttributeBinding {
            binding: binding.inner,
            attribute: attribute.into(),
            value_binding: value_binding.inner,
        });
        value_binding
    }

    pub fn add_attribute_binding_requirement<K>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
        value_binding: BuilderBinding<'bind>,
    )
    where
        K: Into<Symbol>,
    {
        self.select.push(CfgOpSelect::AttributeBinding {
            binding: binding.inner,
            attribute: attribute.into(),
            value_binding: value_binding.inner,
        });
    }

    pub fn add_attribute_requirement<K>(
        &mut self,
        binding: BuilderBinding<'bind>,
        attribute: K,
    )
    where
        K: Into<Symbol>,
    {
        self.select.push(CfgOpSelect::RequireAttribute {
            binding: binding.inner,
            attribute: attribute.into(),
        });
    }

    pub fn add_not_clause<'bind_inner, F>(
        &mut self,
        not_clause_cb: F,
    )
    where
        'bind: 'bind_inner,
        F: FnOnce(&mut SelectBuilder<'_, 'bind_inner>),
    {
        let Self { binding_sequence, select } = self;
        let mut select_builder = SelectBuilder {
            binding_sequence: *binding_sequence,
            select: Vec::new(),
        };
        not_clause_cb(&mut select_builder);
        select.push(CfgOpSelect::Not {
            body: select_builder.select,
        });
    }

    pub fn add_comparison<F>(
        &mut self,
        comparison_cb: F,
    )
    where
        F: FnOnce(CompareBuilder<false, false, false>) -> CompareBuilder<true, true, true>,
    {
        let compare_builder = CompareBuilder {
            op: None,
            left: None,
            right: None,
        };
        let CompareBuilder { op, left, right } = comparison_cb(compare_builder);
        self.select.push(CfgOpSelect::Compare {
            operator: op.expect("builder set compare operator"),
            left: left.expect("builder set left compare value"),
            right: right.expect("builder set right compare value"),
        });
    }

    pub fn add_calculation<F>(
        &mut self,
        calculation_cb: F,
    ) -> BuilderBinding<'bind>
    where
        F: FnOnce(&CalcBuilder) -> CalcBuilderNode,
    {
        let calculation_root = calculation_cb(&CalcBuilder(())).0;
        let binding = self.binding_sequence.next();
        self.select.push(CfgOpSelect::Calculation {
            operation: calculation_root,
            result_binding: binding.inner,
        });
        binding
    }
}

#[derive(Debug)]
pub struct CalcBuilder(());

macro_rules! fn_calc_builder_binop {
    ($name:ident, $op:expr) => {
        pub fn $name(&self, left: CalcBuilderNode, right: CalcBuilderNode) -> CalcBuilderNode {
            CalcBuilderNode(Calculation::BinOp($op, Box::new(left.0), Box::new(right.0)))
        }
    }
}

impl CalcBuilder {

    pub fn value<V>(&self, value: V) -> CalcBuilderNode
    where
        V: Into<Value>,
    {
        CalcBuilderNode(Calculation::Value(value.into()))
    }

    pub fn binding(&self, binding: BuilderBinding<'_>) -> CalcBuilderNode {
        CalcBuilderNode(Calculation::Binding(binding.inner))
    }

    fn_calc_builder_binop!(add, ArithBinOp::Add);
    fn_calc_builder_binop!(subtract, ArithBinOp::Sub);
    fn_calc_builder_binop!(multiply, ArithBinOp::Mul);
    fn_calc_builder_binop!(divide, ArithBinOp::Div);
}

#[derive(Debug, Clone)]
pub struct CalcBuilderNode(Calculation);

#[derive(Debug)]
pub struct CompareBuilder<const OP: bool, const LEFT: bool, const RIGHT: bool> {
    op: Option<CompareOp>,
    left: Option<CompareValue>,
    right: Option<CompareValue>,
}

impl<const OP: bool, const RIGHT: bool> CompareBuilder<OP, false, RIGHT> {

    pub fn left_value<V>(self, value: V) -> CompareBuilder<OP, true, RIGHT>
    where
        V: Into<Value>,
    {
        CompareBuilder {
            op: self.op,
            left: Some(CompareValue::Value(value.into())),
            right: self.right,
        }
    }

    pub fn left_binding(self, binding: BuilderBinding<'_>) -> CompareBuilder<OP, true, RIGHT> {
        CompareBuilder {
            op: self.op,
            left: Some(CompareValue::Binding(binding.inner)),
            right: self.right,
        }
    }
}

impl<const OP: bool, const LEFT: bool> CompareBuilder<OP, LEFT, false> {

    pub fn right_value<V>(self, value: V) -> CompareBuilder<OP, LEFT, true>
    where
        V: Into<Value>,
    {
        CompareBuilder {
            op: self.op,
            left: self.left,
            right: Some(CompareValue::Value(value.into())),
        }
    }

    pub fn right_binding(self, binding: BuilderBinding<'_>) -> CompareBuilder<OP, LEFT, true> {
        CompareBuilder {
            op: self.op,
            left: self.left,
            right: Some(CompareValue::Binding(binding.inner)),
        }
    }
}

macro_rules! fn_cmp_builder_op {
    ($name:ident, $op:expr) => {
        pub fn $name(self) -> CompareBuilder<true, LEFT, RIGHT> {
            CompareBuilder {
                op: Some($op),
                left: self.left,
                right: self.right,
            }
        }
    }
}

impl<const LEFT: bool, const RIGHT: bool> CompareBuilder<false, LEFT, RIGHT> {

    fn_cmp_builder_op!(less, CompareOp::Less);
    fn_cmp_builder_op!(less_or_equal, CompareOp::LessOrEqual);
    fn_cmp_builder_op!(greater, CompareOp::Greater);
    fn_cmp_builder_op!(greater_or_equal, CompareOp::GreaterOrEqual);
    fn_cmp_builder_op!(equal, CompareOp::Equal);
    fn_cmp_builder_op!(not_equal, CompareOp::NotEqual);
}

#[derive(Debug)]
pub struct EnumBuilder<'seq, 'bind> {
    binding_sequence: LinkedBindingSequence<'seq, 'bind>,
    enum_options: Vec<EnumOption>,
}

impl<'seq, 'bind> EnumBuilder<'seq, 'bind> {

    pub fn add_value_option<V>(&mut self, value: V)
    where
        V: Into<Value>,
    {
        self.enum_options.push(EnumOption::Value(value.into()));
    }

    pub fn add_binding_option(&mut self, binding: BuilderBinding<'bind>) {
        self.enum_options.push(EnumOption::Binding(binding.inner));
    }
}

#[derive(Debug)]
pub struct TupleBuilder<'seq, 'bind> {
    binding_sequence: LinkedBindingSequence<'seq, 'bind>,
    tuple_items: Vec<OpenTupleItem>,
}

impl<'seq, 'bind> TupleBuilder<'seq, 'bind> {

    pub fn add_ignored_item(&mut self) {
        self.tuple_items.push(OpenTupleItem::Ignore);
    }

    pub fn add_value_item<V>(&mut self, value: V)
    where
        V: Into<Value>,
    {
        self.tuple_items.push(OpenTupleItem::Compare(value.into()));
    }

    pub fn add_new_binding_item(&mut self) -> BuilderBinding<'bind> {
        let binding = self.binding_sequence.next();
        self.tuple_items.push(OpenTupleItem::Binding(binding.inner));
        binding
    }

    pub fn add_existing_binding_item(&mut self, binding: BuilderBinding<'bind>) {
        self.tuple_items.push(OpenTupleItem::Binding(binding.inner));
    }
}