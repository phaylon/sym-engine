
use sym_engine::*;

#[test]
fn object_bindings() {

    let mut space = Space::new();
    let id_obj = space.create_id();

    let id_ok = space.create_id();
    space.attributes_mut(id_ok).add("value", id_obj);

    let id_err = space.create_id();
    space.attributes_mut(id_err).add("value", 23);

    let mut sys = System::new("test", &["ROOT"]).unwrap();
    sys.build_rule("test", |mut builder, input| {
        let binding = builder.add_attribute_binding(input[0], "value");
        builder.add_object_binding_assertion(binding);
        builder.add_not_clause(|builder| {
            builder.add_attribute_requirement(input[0], "out");
        });
        let mut builder = builder.into_apply_builder();
        builder.add_value_attribute_addition(input[0], "out", "ok");
        builder
    }).unwrap();

    assert!(sys.run_to_first(&mut space, &[id_ok]).unwrap().is_some());
    assert!(sys.run_to_first(&mut space, &[id_err]).unwrap().is_none());
}

#[test]
fn value_comparisons() {

    let mut space = Space::new();
    let root = space.create_id();

    let id_ok = space.create_id();
    space.attributes_mut(id_ok).add("value", 23);

    let id_err = space.create_id();
    space.attributes_mut(id_err).add("value", 42);

    space.attributes_mut(root).apply(|attrs| {
        attrs.add("object", id_ok);
        attrs.add("object", id_err);
    });

    let mut sys = System::new("test", &["ROOT"]).unwrap();
    sys.build_rule("test", |mut builder, input| {
        let object_binding = builder.add_attribute_binding(input[0], "object");
        let value_binding = builder.add_attribute_binding(object_binding, "value");
        builder.add_binding_value_comparison(value_binding, 23);
        builder.add_not_clause(|builder| {
            builder.add_attribute_binding_requirement(input[0], "ok", object_binding);
        });
        let mut builder = builder.into_apply_builder();
        builder.add_binding_attribute_addition(input[0], "ok", object_binding);
        builder
    }).unwrap();

    sys.run_saturation_with_control(&mut space, &[root], control_limit_total(10)).unwrap();
    assert!(space.attributes(root).has("ok", &id_ok));
    assert!(!space.attributes(root).has("ok", &id_err));
}

#[test]
fn tuples() {

    let mut space = Space::new();
    let root = space.create_id();
    space.attributes_mut(root).add("value", vec![
        Value::from("in"),
        Value::from(23),
        Value::from(23),
        Value::from(42),
    ]);
    space.attributes_mut(root).add("value", vec![
        Value::from("in"),
        Value::from(99),
        Value::from(33),
        Value::from(42),
    ]);

    let mut sys = System::new("test", &["ROOT"]).unwrap();
    sys.build_rule("test", |mut builder, input| {
        let binding = builder.add_attribute_binding(input[0], "value");
        let value_binding = builder.add_tuple_unpacking(binding, |builder| {
            builder.add_value_item("in");
            let binding = builder.add_new_binding_item();
            builder.add_existing_binding_item(binding);
            builder.add_ignored_item();
            binding
        });
        builder.add_not_clause(|builder| {
            let binding = builder.add_attribute_binding(input[0], "done");
            builder.add_tuple_unpacking(binding, |builder| {
                builder.add_value_item("out");
                builder.add_existing_binding_item(value_binding);
            });
        });
        let mut builder = builder.into_apply_builder();
        let tuple = builder.add_tuple_creation(|builder| {
            builder.add_value_item("out");
            builder.add_binding_item(value_binding);
        });
        builder.add_binding_attribute_addition(input[0], "done", tuple);
        builder
    }).unwrap();

    sys.run_saturation_with_control(&mut space, &[root], control_limit_total(10)).unwrap();
    let found = space.attributes_mut(root).remove_single_named("done").unwrap();
    assert_eq!(found.tuple().unwrap()[1].int().unwrap(), 23);
    assert!(!space.attributes(root).has_named("done"));
}

#[test]
fn enums() {

    let mut space = Space::new();
    let root = space.create_id();

    let id_ok = space.create_id();
    space.attributes_mut(id_ok).add("value", 23);

    let id_err = space.create_id();
    space.attributes_mut(id_err).add("value", 99);

    space.attributes_mut(root).apply(|attrs| {
        attrs.add("object", id_ok);
        attrs.add("object", id_err);
    });

    let mut sys = System::new("test", &["ROOT"]).unwrap();
    sys.build_rule("test", |mut builder, input| {
        let binding = builder.add_attribute_binding(input[0], "object");
        let value_binding = builder.add_attribute_binding(binding, "value");
        builder.add_enum_match(value_binding, |builder| {
            builder.add_value_option(13);
            builder.add_value_option(23);
            builder.add_value_option(33);
        });
        builder.add_not_clause(|builder| {
            builder.add_attribute_binding_requirement(input[0], "ok", binding);
        });
        let mut builder = builder.into_apply_builder();
        builder.add_binding_attribute_addition(input[0], "ok", binding);
        builder
    }).unwrap();

    sys.run_saturation_with_control(&mut space, &[root], control_limit_total(10)).unwrap();
    let found = space.attributes_mut(root).remove_single_named("ok").unwrap();
    assert_eq!(found.object().unwrap(), id_ok);
    assert!(!space.attributes(root).has_named("ok"));
}

#[test]
fn comparisons() {

    let mut space = Space::new();
    let root = space.create_id();

    space.attributes_mut(root).apply(|attrs| {
        attrs.add("value", 2);
        attrs.add("value", 3);
        attrs.add("value", 4);
    });

    let mut sys = System::new("test", &["ROOT"]).unwrap();
    sys.build_rule("test", |mut builder, input| {
        let binding = builder.add_attribute_binding(input[0], "value");
        builder.add_comparison(|cmp| cmp.less_or_equal().left_binding(binding).right_value(3));
        let mut builder = builder.into_apply_builder();
        builder.add_binding_attribute_removal(input[0], "value", binding, RemovalMode::Required);
        builder.add_binding_attribute_addition(input[0], "ok", binding);
        builder
    }).unwrap();

    sys.run_saturation_with_control(&mut space, &[root], control_limit_total(10)).unwrap();
    assert!(space.attributes(root).has("ok", &2));
    assert!(space.attributes(root).has("ok", &3));
    assert!(!space.attributes(root).has("ok", &4));
}

#[test]
fn calculations() {

    let mut space = Space::new();
    let root = space.create_id();

    space.attributes_mut(root).apply(|attrs| {
        attrs.add("a", 23);
        attrs.add("b", 42);
    });

    let mut sys = System::new("test", &["ROOT"]).unwrap();
    sys.build_rule("test", |mut builder, input| {
        let binding_a = builder.add_attribute_binding(input[0], "a");
        let binding_b = builder.add_attribute_binding(input[0], "b");
        let binding_result = builder.add_calculation(|calc| {
            calc.multiply(
                calc.value(2),
                calc.add(
                    calc.binding(binding_a),
                    calc.binding(binding_b),
                ),
            )
        });
        let mut builder = builder.into_apply_builder();
        builder.add_binding_attribute_removal(input[0], "a", binding_a, RemovalMode::Required);
        builder.add_binding_attribute_removal(input[0], "b", binding_b, RemovalMode::Required);
        builder.add_binding_attribute_addition(input[0], "result", binding_result);
        builder
    }).unwrap();

    sys.run_saturation_with_control(&mut space, &[root], control_limit_total(10)).unwrap();
    assert!(space.attributes(root).has("result", &130));
}

#[test]
fn object_creation() {

    let mut space = Space::new();
    let root = space.create_id();

    let mut sys = System::new("test", &["ROOT"]).unwrap();
    sys.build_rule("test", |mut builder, input| {
        builder.add_not_clause(|builder| {
            builder.add_attribute_requirement(input[0], "value");
        });
        let mut builder = builder.into_apply_builder();
        let binding = builder.add_object_creation();
        builder.add_binding_attribute_addition(input[0], "value", binding);
        builder.add_value_attribute_addition(binding, "done", 23);
        builder
    }).unwrap();

    sys.run_saturation_with_control(&mut space, &[root], control_limit_total(10)).unwrap();
    let value = space.attributes(root).single_named("value").unwrap();
    let object = value.object().unwrap();
    assert!(space.attributes(object).has("done", &23));
}