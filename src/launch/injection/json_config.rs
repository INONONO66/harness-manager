use serde_json::{Map, Value};

pub(super) fn set_dotted_path(node: &mut Value, dotted: &str, leaf: Value) {
    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    let map = node.as_object_mut().expect("object enforced above");
    let (head, rest) = match dotted.split_once('.') {
        Some((h, r)) => (h, Some(r)),
        None => (dotted, None),
    };
    if head.is_empty() {
        return;
    }
    match rest {
        None => {
            map.insert(head.to_string(), leaf);
        }
        Some(rest) => {
            let next = map
                .entry(head.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            set_dotted_path(next, rest, leaf);
        }
    }
}

pub(super) fn merge_provider(root: &mut Value, root_key: &str, provider: &str, value: Value) {
    if !root.is_object() {
        *root = Value::Object(Map::new());
    }
    let container = walk_or_create(root, root_key);
    if !container.is_object() {
        *container = Value::Object(Map::new());
    }
    let map = container.as_object_mut().expect("object enforced above");
    match map.get_mut(provider) {
        Some(existing) if existing.is_object() && value.is_object() => deep_merge(existing, value),
        _ => {
            map.insert(provider.to_string(), value);
        }
    }
}

fn walk_or_create<'a>(node: &'a mut Value, dotted: &str) -> &'a mut Value {
    let (head, rest) = match dotted.split_once('.') {
        Some((h, r)) => (h, Some(r)),
        None => (dotted, None),
    };
    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    let map = node.as_object_mut().expect("object enforced above");
    let next = map
        .entry(head.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    match rest {
        None => next,
        Some(rest) => walk_or_create(next, rest),
    }
}

fn deep_merge(target: &mut Value, source: Value) {
    if let (Some(t_map), Value::Object(s_map)) = (target.as_object_mut(), source.clone()) {
        for (k, v) in s_map {
            match t_map.get_mut(&k) {
                Some(existing) if existing.is_object() && v.is_object() => {
                    deep_merge(existing, v);
                }
                _ => {
                    t_map.insert(k, v);
                }
            }
        }
    } else {
        *target = source;
    }
}
