
use sym_engine::*;
use assert_matches::{assert_matches};

#[track_caller]
fn test_package(rules: &str) -> (System, Space, Id, Id) {

    let mut system = System::new("test", &["A", "B"]).unwrap();
    let mut loader = SystemLoader::new(vec![&mut system]);
    loader.load_str(rules).expect("rules load successful");

    let mut space = Space::new();
    let root_a = space.create_root_id();
    let root_b = space.create_root_id();

    (system, space, root_a, root_b)
}

#[test]
fn input_variable_verification() {

    assert!(matches!(
        System::new("test", &["X", "X"]),
        Err(SystemError::DuplicateInputVariable(var)) if var.as_ref() == "X",
    ));

    assert!(matches!(
        System::new("test", &["*"]),
        Err(SystemError::InvalidInputVariable(name)) if name.as_ref() == "*",
    ));

    assert!(matches!(
        System::new("*", &[]),
        Err(SystemError::InvalidName(name)) if name.as_ref() == "*",
    ));

    let mut space = Space::new();
    assert!(matches!(
        System::new("test", &["X"]).unwrap().run_to_first(&mut space, &[]),
        Err(RuntimeError::InvalidInputArgumentLen { expected: 1, received: 0 }),
    ));
}

#[test]
fn single_rule_fire() {

    // rule found
    let (system, mut space, a, b) = test_package("
        rule test:x {} do { + $A.x: 23 }
        rule test:y {} do { + $A.x: 42 }
    ");
    let fired = system.run_to_first(&mut space, &[a, b]).unwrap();
    assert_eq!(fired.unwrap().as_ref(), "x");
    let value = space.attributes_mut(a).remove_single_named("x").unwrap();
    assert_eq!(value.int(), Some(23));
    assert!(space.attributes(a).is_empty());
    assert!(space.attributes(b).is_empty());

    // no rules applicable
    let (system, mut space, a, b) = test_package("
        rule test:x { $A.flag: true } do { + $A.x: 23 }
        rule test:y { $A.flag: true } do { + $A.x: 42 }
    ");
    assert!(system.run_to_first(&mut space, &[a, b]).unwrap().is_none());
    assert!(space.attributes(a).is_empty());
    assert!(space.attributes(b).is_empty());
}

#[test]
fn saturation() {
    let (system, mut space, a, b) = test_package("
        rule test:move2 { $A.b: $x } do { - $A.b: $x, + $A.c: $x }
        rule test:move1 { $A.in: $x } do { - $A.in: $x, + $A.b: $x }
        rule test:move3 { $A.c: $x } do { - $A.c: $x, + $A.done: $x }
    ");
    space.attributes_mut(a).add("in", 23);
    assert!(!space.attributes(a).is_empty());
    let run_count = system.run_saturation(&mut space, &[a, b]).unwrap();
    assert_eq!(run_count, 3);
    let value = space.attributes_mut(a).remove_single_named("done").unwrap();
    assert_eq!(value.int(), Some(23));
    assert!(space.attributes(a).is_empty());
    assert!(space.attributes(b).is_empty());
}

#[test]
fn rule_saturation() {
    let (system, mut space, a, b) = test_package("
        rule test:move1 {
            $A.val: $v,
        } do {
            - $A.val: $v,
            + $A.buf: $v,
        }
        rule test:move2 {
            $A.buf: $v,
            $nv is $v * 2,
        } do {
            - $A.buf: $v,
            + $A.val: $nv,
        }
    ");
    space.attributes_mut(a).add("val", 23);
    space.attributes_mut(a).add("val", 42);
    let run_count = system.run_rule_saturation(&mut space, &[a, b]).unwrap();
    assert_eq!(run_count, 4);
    let values = space.attributes_mut(a).remove_all_named("val");
    assert_eq!(values.len(), 2);
    assert!(values.contains(&Value::Int(46)));
    assert!(values.contains(&Value::Int(84)));
    assert!(space.attributes(a).is_empty());
    assert!(space.attributes(b).is_empty());
}

#[test]
fn saturation_run_control() {

    let (system, mut space, a, b) = test_package("
        rule test:endless {} do {}
    ");

    let mut self_count = 0;
    let run_result = system.run_saturation_with_control(
        &mut space,
        &[a, b],
        |name, _, count| {
            self_count += 1;
            assert_eq!(name.as_ref(), "endless");
            assert_eq!(count, self_count);
            assert!(count <= 5);
            if count >= 5 {
                RuntimeControl::Stop
            } else {
                RuntimeControl::Continue
            }
        },
    );
    assert_eq!(self_count, 5);
    assert!(matches!(run_result, Err(RuntimeError::Stopped { count: 5 })));
}

#[test]
fn rule_saturation_run_control() {

    let (system, mut space, a, b) = test_package("
        rule test:endless {} do {}
    ");

    let mut self_count = 0;
    let run_result = system.run_rule_saturation_with_control(
        &mut space,
        &[a, b],
        |name, _, count| {
            self_count += 1;
            assert_eq!(name.as_ref(), "endless");
            assert_eq!(count, self_count);
            assert!(count <= 5);
            if count >= 5 {
                RuntimeControl::Stop
            } else {
                RuntimeControl::Continue
            }
        },
    );
    assert_eq!(self_count, 5);
    assert!(matches!(run_result, Err(RuntimeError::Stopped { count: 5 })));
}

#[test]
fn control_helper_limit_total() {
    let (system, mut space, a, b) = test_package("
        rule test:endless {} do {}
    ");
    let run_result = system.run_rule_saturation_with_control(
        &mut space,
        &[a, b],
        control_limit_total(10),
    );
    assert_matches!(run_result, Err(RuntimeError::Stopped { count: 10 }));
}

#[test]
fn control_helper_limit_per_rule() {
    let (system, mut space, a, b) = test_package("
        rule test:a_to_b {} do {
            - $A.val: 23,
            + $B.val: 23,
        }
        rule test:b_to_a {} do {
            - $B.val: 23,
            + $A.val: 23,
        }
    ");
    space.attributes_mut(a).add("val", 23);
    let run_result = system.run_saturation_with_control(
        &mut space,
        &[a, b],
        control_limit_per_rule(10),
    );
    assert_matches!(run_result, Err(RuntimeError::Stopped { count: 19 }));
}

#[test]
fn control_helper_limit_total_and_per_rule() {
    let (system, mut space, a, b) = test_package("
        rule test:endless {
            not { $A.val: $ },
            not { $B.val: $ },
        } do {}
        rule test:a_to_b {} do {
            - $A.val: 23,
            + $B.val: 23,
        }
        rule test:b_to_a {} do {
            - $B.val: 23,
            + $A.val: 23,
        }
    ");
    let run_result = system.run_rule_saturation_with_control(
        &mut space,
        &[a, b],
        control_limit_total(10),
    );
    assert_matches!(run_result, Err(RuntimeError::Stopped { count: 10 }));
    space.attributes_mut(a).add("val", 23);
    let run_result = system.run_saturation_with_control(
        &mut space,
        &[a, b],
        control_limit_per_rule(10),
    );
    assert_matches!(run_result, Err(RuntimeError::Stopped { count: 19 }));
}