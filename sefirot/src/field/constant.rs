use std::sync::Arc;

use parking_lot::Mutex;

use super::*;

pub struct ConstantAccessor<V: Value + Send, T: EmanationType> {
    pub value: Arc<Mutex<V>>,
    _marker: PhantomData<T>,
}

impl<V: Value + Send, T: EmanationType> ConstantAccessor<V, T> {
    pub fn new(value: V) -> Self {
        Self {
            value: Arc::new(Mutex::new(value)),
            _marker: PhantomData,
        }
    }
}

impl<V: Value + Send, T: EmanationType> Accessor<T> for ConstantAccessor<V, T> {
    type V = Expr<V>;
    type C = Expr<V>;

    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        if let Some(expr) = self.get_cache(element, field) {
            Ok(*expr)
        } else {
            let value = self.value.clone();
            let expr = element.context.bind(move || *value.lock());
            self.insert_cache(element, field, expr);
            Ok(expr)
        }
    }
    fn set(
        &self,
        _element: &Element<T>,
        _field: Field<Self::V, T>,
        _value: &Self::V,
    ) -> Result<(), WriteError> {
        Err(WriteError {
            message: "Cannot write to `ConstantAccessor`".to_string(),
        })
    }
    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {
        unreachable!();
    }

    fn can_write(&self) -> bool {
        false
    }
}
