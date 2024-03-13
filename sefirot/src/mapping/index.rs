use crate::internal_prelude::*;

pub struct IndexMap<I: Access, M, T: EmanationType> {
    pub index_field: Field<I, T>,
    pub mapping: M,
}
impl<X, I: Access, M, T: EmanationType> Mapping<X, T::Index> for IndexMap<I, M, T>
where
    M: Mapping<X, I>,
{
    fn access(&self, index: &T::Index, ctx: &mut Context, binding: FieldHandle) -> X {
        let index = self.index_field.at_opt(index, ctx).unwrap();
        self.mapping.access(&index, ctx, binding)
    }
}
