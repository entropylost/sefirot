use super::*;

pub struct IndexMap<J: Access, M, I: FieldIndex> {
    pub index: Field<J, I>,
    pub mapping: M,
}
impl<J: Access, M, I: FieldIndex> IndexMap<J, M, I> {
    pub fn new(index: Field<J, I>, mapping: M) -> Self {
        Self { index, mapping }
    }
}
impl<
        L: AccessList,
        X: Access + ListAccess<List = AccessCons<X, L>>,
        J: Access,
        M,
        I: FieldIndex,
    > Mapping<X, I> for IndexMap<J, M, I>
where
    M: Mapping<X, J>,
    IndexMap<J, M, I>: ListMapping<L, I>,
{
    fn access(&self, index: &I, ctx: &mut Context, binding: FieldId) -> X {
        let index = self.index.id.at_opt(index, ctx).unwrap();
        self.mapping.access(&index, ctx, binding)
    }
    fn save(&self, ctx: &mut Context, binding: FieldId) {
        self.mapping.save(ctx, binding);
    }
}

#[allow(dead_code)]
mod test {
    use luisa::lang::types::vector::{Vec2, Vec4};
    use luisa::lang::types::AtomicRef;

    use self::buffer::{BufferMapping, Tex2dMapping};
    use super::*;
    fn test_mapping<M: Mapping<X, Y>, X: Access, Y: 'static>(_: ()) {}
    fn foo() {
        test_mapping::<
            IndexMap<Expr<u32>, BufferMapping<u32>, Expr<Vec2<u32>>>,
            AtomicRef<u32>,
            Expr<Vec2<u32>>,
        >(());
        test_mapping::<Tex2dMapping<Vec4<f32>>, Var<Vec4<f32>>, Expr<Vec2<u32>>>(());
    }
}
