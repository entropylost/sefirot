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

    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {
        unreachable!();
    }

    fn can_write(&self) -> bool {
        false
    }
}

impl<T: EmanationType> Emanation<T> {
    /// Creates a [`Field`] containing an [`Element`] of another [`Emanation`],
    /// using a pre-existing `Field` containing an integer that is used to
    /// index into the other `Emanation` with the provided [`ArrayIndex`].
    pub fn map_index<S: EmanationType>(
        &self,
        other: &Emanation<S>,
        map: Field<Expr<u32>, T>,
        index: ArrayIndex<S>,
    ) -> Reference<'_, Field<Element<S>, T>> {
        let accessor = IndexMapAccessor {
            map,
            index,
            emanation: other.clone(),
        };
        self.create_field(&format!(
            "map({} => {})",
            pretty_type_name::<T>(),
            pretty_type_name::<S>()
        ))
        .bind(accessor)
    }
}

impl<V: Any + Clone, T: EmanationType> Reference<'_, Field<V, T>> {
    /// Creates a field with the same values by changing the [`Emanation`] using the provided mapping.
    /// Note that the new field is not mutable.
    pub fn over<S: EmanationType>(
        self,
        map: Reference<'_, Field<Element<T>, S>>,
    ) -> Reference<'_, Field<V, S>> {
        map.emanation
            .create_field(&format!(
                "{}-over({})",
                self.name(),
                pretty_type_name::<S>()
            ))
            .bind_fn(move |element| self.value.at(&*map.value.at(element)).clone())
    }
}
