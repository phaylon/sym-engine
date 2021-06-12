
use std::sync::atomic::{AtomicU64, Ordering};
use std::num::{NonZeroU64};
use std::fmt::{Debug};
use fnv::{FnvHashMap};
use crate::{Symbol, Value, MatchValue};

static OBJECT_ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

type ObjectData = FnvHashMap<Id, Vec<(Symbol, Value)>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(NonZeroU64);

impl std::fmt::Display for Id {

    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "<{}>", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct ObjectSet {
    objects: Vec<Id>,
}

impl ObjectSet {

    fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    fn add(&mut self, object: Id) -> bool {
        if self.objects.contains(&object) {
            false
        } else {
            self.objects.push(object);
            true
        }
    }

    fn remove(&mut self, object: Id) -> bool {
        let mut removed = false;
        self.objects.retain(|ex| {
            if *ex == object {
                removed = true;
                false
            } else {
                true
            }
        });
        removed
    }

    fn objects(&self) -> &[Id] {
        &self.objects
    }
}

#[derive(Debug)]
pub struct Space {
    root_objects: ObjectSet,
    objects: ObjectData,
}

impl Space {

    pub fn new() -> Self {
        Self {
            root_objects: ObjectSet::new(),
            objects: ObjectData::default(),
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.objects.retain(|_, values| {
            if values.is_empty() {
                false
            } else {
                values.shrink_to_fit();
                true
            }
        });
    }

    pub fn collect_garbage(&mut self) -> usize {

        let mut marked = Vec::new();
        let roots = self.root_objects
            .objects()
            .iter()
            .copied()
            .map(|id| Value::from(id))
            .collect::<Vec<_>>();
        let mut trace = roots
            .iter()
            .collect::<Vec<_>>();

        'trace: while let Some(trace_value) = trace.pop() {
            let trace_id = match trace_value {
                Value::Object(id) => *id,
                Value::Tuple(values) => {
                    trace.extend(values.iter());
                    continue;
                },
                Value::Symbol(_) |
                Value::Int(_) |
                Value::Float(_) => {
                    continue;
                },
            };
            if let Err(index) = marked.binary_search(&trace_id) {
                marked.insert(index, trace_id);
            } else {
                continue 'trace;
            }
            if let Some(attributes) = self.objects.get(&trace_id) {
                trace.extend(attributes.iter().map(|(_, value)| value));
            }
        }

        let orig_len = self.objects.len();
        self.objects.retain(|id, _| marked.binary_search(id).is_ok());
        orig_len - self.objects.len()
    }
}

impl Access for Space {

    fn create_id(&self) -> Id {
        let id = OBJECT_ID_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let id = NonZeroU64::new(id).expect("available object id");
        Id(id)
    }

    fn register_root(&mut self, object: Id) -> bool {
        self.root_objects.add(object)
    }

    fn unregister_root(&mut self, object: Id) -> bool {
        self.root_objects.remove(object)
    }

    fn attributes(&self, object: Id) -> Attributes<'_> {
        let attributes = self.objects
            .get(&object)
            .map(|attrs| attrs.as_slice())
            .unwrap_or(&[]);
        Attributes::new(object, attributes)
    }

    fn attributes_mut(&mut self, object: Id) -> AttributesMut<'_> {
        let attributes = self.objects
            .entry(object)
            .or_insert_with(Vec::new);
        AttributesMut::new(object, attributes)
    }

    fn transaction(
        &mut self,
        run: &mut dyn for<'tx> FnMut(Transaction<'tx>) -> Option<Transaction<'tx>>,
    ) -> bool {
        let transaction = Transaction::new(self, self.root_objects.clone());
        let maybe_update = run(transaction);
        if let Some(update) = maybe_update {
            let (transaction_root_objects, transaction_objects) = update.unpack();
            self.root_objects = transaction_root_objects;
            for (id, attributes) in transaction_objects {
                self.objects.insert(id, attributes);
            }
            true
        } else {
            false
        }
    }

    fn roots(&self) -> &[Id] {
        self.root_objects.objects()
    }
}

pub trait Access: Debug {

    fn create_id(&self) -> Id;

    fn create_root_id(&mut self) -> Id {
        let id = self.create_id();
        self.register_root(id);
        id
    }

    fn create_object(&mut self) -> AttributesMut<'_> {
        let id = self.create_id();
        self.attributes_mut(id)
    }

    fn create_root_object(&mut self) -> AttributesMut<'_> {
        let id = self.create_root_id();
        self.attributes_mut(id)
    }

    fn register_root(&mut self, object: Id) -> bool;

    fn unregister_root(&mut self, object: Id) -> bool;

    fn roots(&self) -> &[Id];

    fn attributes(&self, object: Id) -> Attributes<'_>;

    fn attributes_mut(&mut self, object: Id) -> AttributesMut<'_>;

    fn transaction(
        &mut self,
        body: &mut dyn for<'tx> FnMut(Transaction<'tx>) -> Option<Transaction<'tx>>,
    ) -> bool;
}

#[derive(Debug, Clone)]
pub struct AttributesIter<'a> {
    attributes: &'a [(Symbol, Value)],
}

impl<'a> Iterator for AttributesIter<'a> {

    type Item = (&'a Symbol, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(((name, value), rest)) = self.attributes.split_first() {
            self.attributes = rest;
            Some((name, value))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Attributes<'a> {
    object: Id,
    attributes: &'a [(Symbol, Value)],
}

impl<'a> Attributes<'a> {

    fn new(object: Id, attributes: &'a [(Symbol, Value)]) -> Self {
        Self { object, attributes }
    }

    pub fn object(&self) -> Id {
        self.object
    }

    pub fn len(&self) -> usize {
        self.attributes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }

    pub fn to_vec(&self) -> Vec<(Symbol, Value)> {
        self.attributes.to_vec()
    }

    pub fn has<M>(&self, name: &str, value: &M) -> bool
    where
        M: MatchValue + ?Sized,
    {
        self.attributes.iter().any(|(ex_name, ex_value)| {
            ex_name.as_ref() == name
            &&
            value.match_value(ex_value)
        })
    }

    pub fn has_named(&self, name: &str) -> bool {
        self.attributes.iter().any(|(ex_name, _)| ex_name.as_ref() == name)
    }

    pub fn iter(&self) -> AttributesIter<'a> {
        AttributesIter { attributes: self.attributes }
    }

    pub fn iter_named<'n>(&self, name: &'n str) -> impl Iterator<Item = &'a Value> + 'n
    where
        'a: 'n,
    {
        self.attributes.iter().filter_map(move |(ex_name, value)| {
            if ex_name.as_ref() == name {
                Some(value)
            } else {
                None
            }
        })
    }

    pub fn first_named(&self, name: &str) -> Option<&'a Value> {
        self.iter_named(name).next()
    }

    pub fn apply<F, R>(&self, apply: F) -> R
    where
        F: FnOnce(&Self) -> R,
    {
        apply(self)
    }
}

#[derive(Debug)]
pub struct AttributesMut<'a> {
    object: Id,
    attributes: &'a mut Vec<(Symbol, Value)>,
}

impl<'a> AttributesMut<'a> {

    fn new(object: Id, attributes: &'a mut Vec<(Symbol, Value)>) -> Self {
        Self { object, attributes }
    }

    pub fn object(&self) -> Id {
        self.object
    }

    pub fn inspect(&'a self) -> Attributes<'a> {
        Attributes::new(self.object, self.attributes)
    }

    pub fn add<S, V>(&mut self, name: S, value: V)
    where
        S: Into<Symbol>,
        V: Into<Value>,
    {
        self.attributes.push((name.into(), value.into()));
    }

    pub fn remove_first<M>(&mut self, name: &str, value: &M) -> Option<(Symbol, Value)>
    where
        M: MatchValue + ?Sized,
    {
        let maybe_index = self.attributes.iter().position(|(ex_name, ex_value)| {
            ex_name.as_ref() == name && value.match_value(ex_value)
        });
        if let Some(index) = maybe_index {
            Some(self.attributes.remove(index))
        } else {
            None
        }
    }

    pub fn remove_first_named(&mut self, name: &str) -> Option<(Symbol, Value)> {
        let maybe_index = self.attributes
            .iter()
            .position(|(ex_name, _)| ex_name.as_ref() == name);
        if let Some(index) = maybe_index {
            Some(self.attributes.remove(index))
        } else {
            None
        }
    }

    pub fn remove_all_named(&mut self, name: &str) -> Vec<Value> {
        let mut values = Vec::new();
        while let Some((_, value)) = self.remove_first_named(name) {
            values.push(value);
        }
        values
    }

    pub fn retain<F>(&mut self, mut should_retain: F) -> usize
    where
        F: FnMut(&Symbol, &Value) -> bool,
    {
        let orig_len = self.attributes.len();
        self.attributes.retain(|(name, value)| should_retain(name, value));
        orig_len - self.attributes.len()
    }

    pub fn retain_named(&mut self, name: &str) -> usize {
        self.retain(|ex_name, _| ex_name.as_ref() == name)
    }

    pub fn clear_all(&mut self) -> usize {
        let orig_len = self.attributes.len();
        self.attributes.clear();
        orig_len
    }

    pub fn clear_named(&mut self, name: &str) -> usize {
        self.retain(|ex_name, _| ex_name.as_ref() != name)
    }

    pub fn apply<F, R>(&mut self, apply: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        apply(self)
    }
}

#[derive(Debug, Clone)]
pub struct Transaction<'a> {
    outer: &'a dyn Access,
    local_root_objects: ObjectSet,
    local_objects: ObjectData,
}

impl<'a> Transaction<'a> {

    fn new(outer: &'a dyn Access, local_root_objects: ObjectSet) -> Self {
        Self {
            outer,
            local_root_objects,
            local_objects: ObjectData::default(),
        }
    }

    fn unpack(self) -> (ObjectSet, ObjectData) {
        (self.local_root_objects, self.local_objects)
    }
}

impl<'a> Access for Transaction<'a> {

    fn create_id(&self) -> Id {
        self.outer.create_id()
    }

    fn register_root(&mut self, object: Id) -> bool {
        self.local_root_objects.add(object)
    }

    fn unregister_root(&mut self, object: Id) -> bool {
        self.local_root_objects.remove(object)
    }

    fn attributes(&self, object: Id) -> Attributes<'_> {
        self.local_objects
            .get(&object)
            .map(|attrs| attrs.as_slice())
            .map(|attrs| Attributes::new(object, attrs))
            .unwrap_or_else(|| self.outer.attributes(object))
    }

    fn attributes_mut(&mut self, object: Id) -> AttributesMut<'_> {
        let Self { ref mut local_objects, outer, .. } = *self;
        let attributes = local_objects
            .entry(object)
            .or_insert_with(|| outer.attributes(object).to_vec());
        AttributesMut::new(object, attributes)
    }

    fn transaction(
        &mut self,
        run: &mut dyn for<'tx> FnMut(Transaction<'tx>) -> Option<Transaction<'tx>>,
    ) -> bool {
        let transaction = Transaction::new(self, self.local_root_objects.clone());
        let maybe_update = run(transaction);
        if let Some(update) = maybe_update {
            let (transaction_root_objects, transaction_objects) = update.unpack();
            self.local_root_objects = transaction_root_objects;
            for (id, attributes) in transaction_objects {
                self.local_objects.insert(id, attributes);
            }
            true
        } else {
            false
        }
    }

    fn roots(&self) -> &[Id] {
        self.local_root_objects.objects()
    }
}

