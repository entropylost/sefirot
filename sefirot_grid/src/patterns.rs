use std::sync::Arc;

use luisa::lang::types::vector::Vec2;
use sefirot::ext_prelude::*;
use sefirot::mapping::function::FnMapping;

use crate::GridDomain;

#[derive(Debug, Clone)]
pub struct CheckerboardPattern {
    pub(crate) grid: GridDomain,
}

impl DomainImpl for CheckerboardPattern {
    type Args = ();
    type Index = Expr<Vec2<i32>>;
    type Passthrough = bool;
    #[tracked_nc]
    fn get_element(
        &self,
        kernel_context: Arc<KernelContext>,
        parity: Expr<bool>,
    ) -> Element<Self::Index> {
        let uindex = Vec2::expr(
            dispatch_id().x,
            dispatch_id().y * 2 + (dispatch_id().x + parity.cast::<u32>()) % 2,
        );
        let index = uindex.cast_i32() + Vec2::from(self.grid.start);
        let mut context = Context::new(kernel_context);
        context.bind_local(self.grid.index, FnMapping::new(move |_el, _ctx| uindex));
        Element::new(index, context)
    }
    fn dispatch(
        &self,
        _: Self::Args,
        args: KernelDispatch<Self::Passthrough>,
    ) -> NodeConfigs<'static> {
        let [w, h] = self.grid.size();
        (
            args.dispatch_with([w, h / 2, 1], false),
            args.dispatch_with([w, h / 2, 1], true),
        )
            .chain()
    }
    fn contains_impl(&self, index: &Self::Index) -> Expr<bool> {
        self.grid.contains(index)
    }
}

/// A pattern that invokes the function on 2x2 blocks separately.
/// See the [Margolus neighborhood](https://en.wikipedia.org/wiki/Block_cellular_automaton).
#[derive(Debug, Clone)]
pub struct MargolusPattern {
    pub(crate) grid: GridDomain,
}

impl DomainImpl for MargolusPattern {
    type Args = ();
    type Index = Expr<Vec2<i32>>;
    type Passthrough = Vec2<bool>;
    #[tracked_nc]
    fn get_element(
        &self,
        kernel_context: Arc<KernelContext>,
        offset: Expr<Vec2<bool>>,
    ) -> Element<Self::Index> {
        let uindex = dispatch_id().xy() * 2 + offset.cast::<u32>();
        let index = uindex.cast_i32() + Vec2::from(self.grid.start);
        let mut context = Context::new(kernel_context);
        context.bind_local(self.grid.index, FnMapping::new(move |_el, _ctx| uindex));
        Element::new(index, context)
    }
    fn dispatch(
        &self,
        _: Self::Args,
        args: KernelDispatch<Self::Passthrough>,
    ) -> NodeConfigs<'static> {
        let [w, h] = self.grid.size();
        if self.grid.wrapping {
            let size = [w / 2, h / 2, 1];
            (
                args.dispatch_with(size, Vec2::new(false, false)),
                args.dispatch_with(size, Vec2::new(false, true)),
                args.dispatch_with(size, Vec2::new(true, true)),
                args.dispatch_with(size, Vec2::new(true, false)),
            )
                .chain()
        } else {
            (
                args.dispatch_with([w / 2, h / 2, 1], Vec2::new(false, false)),
                args.dispatch_with([w / 2, (h - 1) / 2, 1], Vec2::new(false, true)),
                args.dispatch_with([(w - 1) / 2, (h - 1) / 2, 1], Vec2::new(true, true)),
                args.dispatch_with([(w - 1) / 2, h / 2, 1], Vec2::new(true, false)),
            )
                .chain()
        }
    }
    fn contains_impl(&self, index: &Self::Index) -> Expr<bool> {
        self.grid.contains(index)
    }
}
