
use sym_engine::*;

#[test]
fn roots() {
    let mut space = Space::new();
    let obj_a = space.create_id();
    let obj_b = space.create_id();
    space.register_root(obj_a);
    assert!(space.roots().contains(&obj_a));
    assert!(!space.roots().contains(&obj_b));
    let obj_c = space.create_root_id();
    assert!(space.roots().contains(&obj_c));
    space.unregister_root(obj_a);
    assert!(!space.roots().contains(&obj_a));
}

#[test]
fn garbage_collection() {
    let mut space = Space::new();

    let mark_object = |attrs: &mut AttributesMut<'_>| {
        attrs.add("mark", "ex");
        attrs.object()
    };

    let obj_root = space.create_root_object().apply(mark_object);
    let obj_direct = space.create_object().apply(mark_object);
    let obj_tuple_a = space.create_object().apply(mark_object);
    let obj_tuple_b = space.create_object().apply(mark_object);
    let obj_dangle_a = space.create_object().apply(mark_object);
    let obj_dangle_b = space.create_object().apply(mark_object);

    space.attributes_mut(obj_direct).add("tuple", vec![obj_tuple_a, obj_tuple_b]);
    space.attributes_mut(obj_root).add("direct", obj_direct);
    space.attributes_mut(obj_tuple_a).add("backlink", obj_root);
    space.attributes_mut(obj_dangle_a).add("b", obj_dangle_b);

    assert!(space.attributes(obj_dangle_a).has("mark", "ex"));
    assert!(space.attributes(obj_dangle_b).has("mark", "ex"));

    assert_eq!(space.collect_garbage(), 2);

    assert!(!space.attributes(obj_dangle_a).has("mark", "ex"));
    assert!(!space.attributes(obj_dangle_b).has("mark", "ex"));

    assert!(space.attributes(obj_root).has("mark", "ex"));
    assert!(space.attributes(obj_direct).has("mark", "ex"));
    assert!(space.attributes(obj_tuple_a).has("mark", "ex"));
    assert!(space.attributes(obj_tuple_b).has("mark", "ex"));
}

mod attributes {
    use super::*;

    #[test]
    fn add() {
        let mut space = Space::new();
        let obj_a = space.create_id();
        assert!(space.attributes(obj_a).is_empty());
        space.attributes_mut(obj_a).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("bar", 2);
            attrs.add("bar", 3);
        });
        assert!(space.attributes(obj_a).has("foo", &23));
        assert!(!space.attributes(obj_a).has("foo", &2));
    }

    #[test]
    fn inspect() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).add("foo", 23);
        assert!(space.attributes_mut(obj).inspect().has("foo", &23));
    }

    #[test]
    fn remove_first() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("foo", 23);
        });
        assert_eq!(space.attributes(obj).len(), 2);
        let value = space.attributes_mut(obj).remove_first("foo", &23).unwrap();
        assert_eq!(value, 23.into());
        assert_eq!(space.attributes(obj).len(), 1);
        assert!(space.attributes(obj).has("foo", &23));
    }

    #[test]
    fn remove_first_named() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("foo", 23);
        });
        assert_eq!(space.attributes(obj).len(), 2);
        let value = space.attributes_mut(obj).remove_first_named("foo").unwrap();
        assert_eq!(value, 23.into());
        assert_eq!(space.attributes(obj).len(), 1);
        assert!(space.attributes(obj).has("foo", &23));
    }

    #[test]
    fn retain() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("bar", 42);
            attrs.add("qux", 99);
        });
        assert_eq!(space.attributes_mut(obj).retain(|_name, value| value != &42.into()), 1);
        assert_eq!(space.attributes(obj).len(), 2);
        assert!(space.attributes(obj).has("foo", &23));
        assert!(space.attributes(obj).has("qux", &99));
    }

    #[test]
    fn retain_named() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("bar", 42);
            attrs.add("qux", 99);
        });
        assert_eq!(space.attributes_mut(obj).retain_named("bar"), 2);
        assert_eq!(space.attributes(obj).len(), 1);
        assert!(space.attributes(obj).has("bar", &42));
    }

    #[test]
    fn clear_all() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("bar", 42);
            attrs.add("qux", 99);
        });
        assert_eq!(space.attributes(obj).len(), 3);
        assert_eq!(space.attributes_mut(obj).clear_all(), 3);
        assert_eq!(space.attributes(obj).len(), 0);
    }

    #[test]
    fn clear_named() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("bar", 42);
            attrs.add("qux", 99);
        });
        assert_eq!(space.attributes(obj).len(), 3);
        assert_eq!(space.attributes_mut(obj).clear_named("bar"), 1);
        assert_eq!(space.attributes(obj).len(), 2);
        assert!(space.attributes(obj).has("foo", &23));
        assert!(space.attributes(obj).has("qux", &99));
    }

    #[test]
    fn iter() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("bar", 42);
            attrs.add("qux", 99);
        });
        let mut iter = space.attributes(obj).iter();
        assert_eq!(iter.next(), Some((&Symbol::from("foo"), &Value::from(23))));
        assert_eq!(iter.next(), Some((&Symbol::from("bar"), &Value::from(42))));
        assert_eq!(iter.next(), Some((&Symbol::from("qux"), &Value::from(99))));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_named() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("bar", 42);
            attrs.add("foo", 99);
        });
        let mut iter = space.attributes(obj).iter_named("foo");
        assert_eq!(iter.next(), Some(&Value::from(23)));
        assert_eq!(iter.next(), Some(&Value::from(99)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn remove_all_named() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("foo", 23);
            attrs.add("foo", 42);
        });
        let values = space.attributes_mut(obj).remove_all_named("foo");
        assert!(values.contains(&Value::Int(23)));
        assert!(values.contains(&Value::Int(42)));
        assert!(space.attributes(obj).is_empty());
    }

    #[test]
    fn first_named() {
        let mut space = Space::new();
        let obj = space.create_id();
        space.attributes_mut(obj).apply(|attrs| {
            attrs.add("bar", 42);
            attrs.add("foo", 23);
            attrs.add("foo", 99);
        });
        assert_eq!(space.attributes(obj).first_named("foo"), Some(&Value::from(23)));
        assert_eq!(space.attributes(obj).first_named("qux"), None);
    }
}

mod transactions {
    use super::*;

    #[test]
    fn commit() {
        let mut space = Space::new();

        let obj_nomod_data = space.create_object().apply(|attrs| {
            attrs.add("data", 23);
            attrs.object()
        });

        let obj_mod_empty = space.create_id();
        let obj_mod_data = space.create_object().apply(|attrs| {
            attrs.add("data", 23);
            attrs.object()
        });

        let obj_root_rm = space.create_root_id();
        let obj_root_stay = space.create_root_id();
        let obj_root_new = space.create_id();

        let mut maybe_obj_new = None;

        assert!(space.transaction(&mut |mut tx| {
            tx.attributes_mut(obj_mod_empty).add("mod", 42);
            tx.attributes_mut(obj_mod_data).add("mod", 42);
            tx.unregister_root(obj_root_rm);
            tx.register_root(obj_root_new);
            let obj_new = tx.create_id();
            maybe_obj_new = Some(obj_new);
            tx.attributes_mut(obj_new).add("mod", 42);
            Some(tx)
        }));

        assert!(!space.roots().contains(&obj_root_rm));
        assert!(space.roots().contains(&obj_root_stay));
        assert!(space.roots().contains(&obj_root_new));

        assert!(space.attributes(obj_mod_empty).has("mod", &42));
        assert!(space.attributes(obj_mod_data).has("mod", &42));

        assert!(space.attributes(obj_nomod_data).has("data", &23));
        assert!(space.attributes(obj_mod_empty).has("mod", &42));
        assert!(space.attributes(obj_mod_data).has("mod", &42));

        let obj_new = maybe_obj_new.unwrap();
        assert!(space.attributes(obj_new).has("mod", &42));
    }

    #[test]
    fn rollback() {
        let mut space = Space::new();

        let obj_nomod_data = space.create_object().apply(|attrs| {
            attrs.add("data", 23);
            attrs.object()
        });

        let obj_mod_empty = space.create_id();
        let obj_mod_data = space.create_object().apply(|attrs| {
            attrs.add("data", 23);
            attrs.object()
        });

        let obj_root_rm = space.create_root_id();
        let obj_root_stay = space.create_root_id();
        let obj_root_new = space.create_id();

        let mut maybe_obj_new = None;

        assert!(!space.transaction(&mut |mut tx| {
            tx.attributes_mut(obj_mod_empty).add("mod", 42);
            tx.attributes_mut(obj_mod_data).add("mod", 42);
            tx.unregister_root(obj_root_rm);
            tx.register_root(obj_root_new);
            let obj_new = tx.create_id();
            maybe_obj_new = Some(obj_new);
            tx.attributes_mut(obj_new).add("mod", 42);
            None
        }));

        assert!(space.roots().contains(&obj_root_rm));
        assert!(space.roots().contains(&obj_root_stay));
        assert!(!space.roots().contains(&obj_root_new));

        assert!(!space.attributes(obj_mod_empty).has("mod", &42));
        assert!(!space.attributes(obj_mod_data).has("mod", &42));

        assert!(space.attributes(obj_nomod_data).has("data", &23));
        assert!(!space.attributes(obj_mod_empty).has("mod", &42));
        assert!(!space.attributes(obj_mod_data).has("mod", &42));

        let obj_new = maybe_obj_new.unwrap();
        assert!(!space.attributes(obj_new).has("mod", &42));
    }
}