use std::collections::HashMap;

use super::*;

pub enum Param<T: PartialEq + PartialOrd> {
    Eq(T),
    Neq(T),
    Lt(T),
    Lte(T),
    Gt(T),
    Gte(T),
}

pub struct Query {
    // fields: HashMap<String, Value>
}
