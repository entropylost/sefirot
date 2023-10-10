use super::array::ArrayIndex;
use super::*;

pub struct IndexMapAccessor<T: EmanationType, S: EmanationType> {
    map: Field<Expr<u32>, T>,
    index: ArrayIndex<S>,
    emanation: Emanation<S>,
}

impl<T: EmanationType, S: EmanationType> Accessor<T> for IndexMapAccessor<T, S> {
    type V = Element<S>;
    type C = ();

    fn get(&self, element: &Element<T>, _field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        let index = element.get(self.map)?;
        Ok(self.emanation.get(&element.context, &self.index, index))
    }

    fn set(
        &self,
        _element: &Element<T>,
        _field: Field<Self::V, T>,
        _value: &Self::V,
    ) -> Result<(), WriteError> {
        Err(WriteError {
            message: "Cannot write to `IndexMapAccessor`".to_string(),
        })
    }

    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {}

    fn can_write(&self) -> bool {
        false
    }
}

impl<T: EmanationType> Emanation<T> {
    pub fn map_index<S: EmanationType>(
        &self,
        other: &Emanation<S>,
        map: Field<Expr<u32>, T>,
        index: ArrayIndex<S>,
    ) -> Field<Element<S>, T> {
        let accessor = IndexMapAccessor {
            map,
            index,
            emanation: other.clone(),
        };
        self.create_bound_field(
            Some(format!(
                "Mapping {} -> {}",
                pretty_type_name::<T>(),
                pretty_type_name::<S>()
            )),
            accessor,
        )
    }
}
