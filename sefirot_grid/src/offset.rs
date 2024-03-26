use std::rc::Rc;

use luisa::lang::types::vector::Vec2;
use luisa::runtime::KernelArg;
use sefirot::ext_prelude::*;
use sefirot::mapping::function::FnMapping;
use sefirot::mapping::index::IndexMap;

use crate::tiled::TileDomain;
use crate::Cell;

#[derive(Clone)]
pub struct OffsetDomain<D: DomainImpl<Index = Expr<Vec2<u32>>>> {
    pub domain: D,
    pub offset: Vec2<i32>,
    pub index: Option<EField<Vec2<u32>, Cell>>,
}

impl<D: DomainImpl<Index = Expr<Vec2<u32>>>> DomainImpl for OffsetDomain<D> {
    type Args = D::Args;
    type Index = Cell;
    type Passthrough = D::Passthrough;
    #[tracked_nc]
    fn get_element(
        &self,
        kernel_context: Rc<KernelContext>,
        passthrough: <Self::Passthrough as KernelArg>::Parameter,
    ) -> Element<Self::Index> {
        let el = self.domain.get_element(kernel_context, passthrough);
        if let Some(index) = self.index {
            let idx = *el;
            el.context().bind_local(
                index,
                FnMapping::<Expr<Vec2<u32>>, Cell, _>::new(move |_, _| idx),
            );
        }
        el.map_index(|idx| idx.cast_i32() + self.offset)
    }
    fn dispatch(
        &self,
        domain_args: Self::Args,
        args: KernelDispatch<Self::Passthrough>,
    ) -> NodeConfigs<'static> {
        self.domain.dispatch(domain_args, args)
    }
    #[tracked_nc]
    fn contains_impl(&self, index: &Self::Index) -> Expr<bool> {
        (index >= self.offset).all() && self.domain.contains_impl(&(index - self.offset).cast_u32())
    }
}

impl OffsetDomain<TileDomain> {
    #[tracked]
    pub fn activate(&self, el: &Element<Cell>) {
        self.domain
            .activate(&el.at((**el - self.offset).cast_u32()))
    }
    #[tracked]
    pub fn active(&self) -> impl Mapping<Expr<bool>, Expr<Vec2<i32>>> {
        IndexMap::new(self.index.unwrap(), self.domain.active())
    }
}
