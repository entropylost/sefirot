use sefirot::ext_prelude::*;
use sefirot::field::{Access, FieldHandle};
use sefirot::luisa::lang::types::vector::Vec2;
use sefirot::luisa::lang::types::AtomicRef;
use sefirot::mapping::buffer::{HandledBuffer, IntoHandled, StaticDomain};
use sefirot::mapping::function::CachedFnMapping;
use sefirot::mapping::index::IndexMap;

#[derive(Debug)]
pub struct LinearEncoder {
    index: EEField<u32, Vec2<u32>>,
    _handle: Option<FieldHandle>,
    max_size: u32,
}
impl Clone for LinearEncoder {
    fn clone(&self) -> Self {
        Self {
            index: self.index,
            _handle: None,
            max_size: self.max_size,
        }
    }
}
impl LinearEncoder {
    pub fn index(&self) -> EEField<u32, Vec2<u32>> {
        self.index
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
            max_size: 1 << 16,
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

                z | ((z >> 16) << 1)
            })),
        );
        Self {
            index,
            _handle: Some(handle),
            max_size: 1 << 8,
        }
    }
    pub fn from_lut(texture: Tex2d<u32>) -> Self {
        debug_assert_eq!(texture.width(), texture.height());
        let size = texture.width();
        debug_assert!(size.is_power_of_two());
        let (index, handle) = Field::create_bind(
            "linear-encoder-lookup",
            StaticDomain::<2>::new(size, size).map_tex2d(texture),
        );
        Self {
            index,
            _handle: Some(handle),
            max_size: size,
        }
    }
    pub fn allowed_size(&self, size: [u32; 2]) -> bool {
        size[0] == size[1] && size[0] <= self.max_size && size[0].is_power_of_two()
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
        device: &Device,
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
        device: &Device,
    ) -> impl AEMapping<V, Vec2<u32>> {
        self.map_buffer_encoded(
            encoder,
            device.create_buffer((self.width() * self.height()) as usize),
        )
    }
}
