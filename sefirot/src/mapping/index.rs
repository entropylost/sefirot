use super::*;

pub struct IndexMap<I: Access, M, T: EmanationType> {
    pub index_field: Field<I, T>,
    pub mapping: M,
}
impl<
        L: AccessList,
        X: Access + ListAccess<List = AccessCons<X, L>>,
        I: Access,
        M,
        T: EmanationType,
    > Mapping<X, T::Index> for IndexMap<I, M, T>
where
    M: Mapping<X, I>,
    IndexMap<I, M, T>: ListMapping<L, T::Index>,
{
    fn access(&self, index: &T::Index, ctx: &mut Context, binding: FieldHandle) -> X {
        let index = self.index_field.at_opt(index, ctx).unwrap();
        self.mapping.access(&index, ctx, binding)
    }
}

mod test {
    use luisa::lang::types::vector::Vec2;

    use self::buffer::BufferMapping;
    use self::cache::CachedMapping;
    use super::*;
    use crate::emanation::Auto;
    pub type E = Auto<Expr<Vec2<u32>>>;
    fn test_mapping<M: Mapping<X, Y>, X: Access, Y: 'static>(_: ()) {}
    fn foo() {
        test_mapping::<
            IndexMap<Expr<u32>, CachedMapping<BufferMapping<u32>>, E>,
            Expr<u32>,
            Expr<Vec2<u32>>,
        >(());
    }
}
