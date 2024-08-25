use std::fmt::Debug;
use std::sync::Arc;

use sefirot::ext_prelude::*;
use sefirot::field::{Access, FieldHandle};
use sefirot::luisa::lang::types::vector::Vec2;
use sefirot::luisa::lang::types::AtomicRef;
use sefirot::mapping::buffer::{HandledBuffer, IntoHandled, StaticDomain};
use sefirot::mapping::function::CachedFnMapping;
use sefirot::mapping::index::IndexMap;

pub struct LinearEncoder {
    index: EEField<u32, Vec2<u32>>,
    _handle: Option<FieldHandle>,
    size_test: Arc<Box<dyn Fn([u32; 2]) -> bool + Send + Sync>>,
}
impl Debug for LinearEncoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearEncoder")
            .field("index", &self.index)
            .field("_handle", &self._handle)
            .finish()
    }
}
impl Clone for LinearEncoder {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            _handle: None,
            size_test: self.size_test.clone(),
        }
    }
}
impl LinearEncoder {
    pub fn index(&self) -> EEField<u32, Vec2<u32>> {
        self.index
    }
    pub fn row_major(x_size: u32) -> Self {
        let (index, handle) = Field::create_bind(
            "linear-encoder-row-major",
            CachedFnMapping::<Expr<u32>, Expr<Vec2<u32>>, _>::new(track_nc!(move |index, _ctx| {
                index.x * x_size + index.y
            })),
        );
        Self {
            index,
            _handle: Some(handle),
            size_test: Arc::new(Box::new(move |size| size[0] == x_size)),
        }
    }
    pub fn morton() -> Self {
        let (index, handle) = Field::create_bind(
            "linear-encoder-morton",
            CachedFnMapping::<Expr<u32>, Expr<Vec2<u32>>, _>::new(track_nc!(move |index, _ctx| {
                // https://graphics.stanford.edu/%7Eseander/bithacks.html#InterleaveBMN

                let x = index.x.var();

                *x = (x | (x << 8)) & 0x00ff00ff;
                *x = (x | (x << 4)) & 0x0f0f0f0f; // 0b00001111
                *x = (x | (x << 2)) & 0x33333333; // 0b00110011
                *x = (x | (x << 1)) & 0x55555555; // 0b01010101

                let y = index.y.var();

                *y = (y | (y << 8)) & 0x00ff00ff;
                *y = (y | (y << 4)) & 0x0f0f0f0f; // 0b00001111
                *y = (y | (y << 2)) & 0x33333333; // 0b00110011
                *y = (y | (y << 1)) & 0x55555555; // 0b01010101

                x | (y << 1)
            })),
        );
        Self {
            index,
            _handle: Some(handle),
            size_test: Arc::new(Box::new(move |size| {
                size[0] == size[1] && size[0] <= 1 << 16 && size[0].is_power_of_two()
            })),
        }
    }
    // Requires the inputs to be within 0..256.
    pub fn morton_256() -> Self {
        let (index, handle) = Field::create_bind(
            "linear-encoder-morton-256",
            CachedFnMapping::<Expr<u32>, Expr<Vec2<u32>>, _>::new(track_nc!(move |index, _ctx| {
                // https://graphics.stanford.edu/%7Eseander/bithacks.html#InterleaveBMN
                // https://docs.rs/morton/0.3.0/src/morton/lib.rs.html

                let z = (index.y << 16 | index.x).var();

                *z = (z | (z << 4)) & 0x0f0f0f0f; // 0b00001111
                *z = (z | (z << 2)) & 0x33333333; // 0b00110011
                *z = (z | (z << 1)) & 0x55555555; // 0b01010101

                (z & (1_u32 << 16) - 1_u32) | ((z >> 16) << 1)
            })),
        );
        Self {
            index,
            _handle: Some(handle),
            size_test: Arc::new(Box::new(move |size| {
                size[0] == size[1] && size[0] <= 1 << 8 && size[0].is_power_of_two()
            })),
        }
    }
    pub fn from_lut(texture: Tex2d<u32>) -> Self {
        let w = texture.width();
        let h = texture.height();
        let (index, handle) = Field::create_bind(
            "linear-encoder-lookup",
            StaticDomain::<2>::new(w, h).map_tex2d(texture),
        );
        Self {
            index,
            _handle: Some(handle),
            size_test: Arc::new(Box::new(move |size| size[0] == w && size[1] == h)),
        }
    }
    pub fn allowed_size(&self, size: [u32; 2]) -> bool {
        (self.size_test)(size)
    }
    pub fn encode<X: Access, M: Mapping<X, Expr<u32>>>(
        &self,
        mapping: M,
    ) -> IndexMap<Expr<u32>, M, Expr<Vec2<u32>>> {
        IndexMap::new(self.index, mapping)
    }
}
mod private {
    use super::*;
    pub trait Sealed {}
    impl Sealed for StaticDomain<2> {}
}
pub trait StaticDomainExt: private::Sealed {
    fn map_buffer_encoded<V: Value>(
        &self,
        encoder: &LinearEncoder,
        buffer: impl IntoHandled<H = HandledBuffer<V>>,
    ) -> impl AEMapping<V, Vec2<u32>>;
    fn create_buffer_encoded<V: Value>(
        &self,
        encoder: &LinearEncoder,
    ) -> impl AEMapping<V, Vec2<u32>>;
}
impl StaticDomainExt for StaticDomain<2> {
    fn map_buffer_encoded<V: Value>(
        &self,
        encoder: &LinearEncoder,
        buffer: impl IntoHandled<H = HandledBuffer<V>>,
    ) -> impl AEMapping<V, Vec2<u32>> {
        debug_assert!(encoder.allowed_size(self.0));
        encoder.encode::<AtomicRef<V>, _>(
            StaticDomain::<1>::new(self.width() * self.height()).map_buffer(buffer),
        )
    }
    fn create_buffer_encoded<V: Value>(
        &self,
        encoder: &LinearEncoder,
    ) -> impl AEMapping<V, Vec2<u32>> {
        self.map_buffer_encoded(
            encoder,
            device().create_buffer((self.width() * self.height()) as usize),
        )
    }
}
