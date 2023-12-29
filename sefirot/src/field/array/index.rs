use super::*;

pub trait LinearIndex<T: EmanationType>: Deref<Target = EField<u32, T>> + Send + Sync {
    fn size(&self) -> u32;
    fn reduce(&self) -> ReducedIndex<T> {
        ReducedIndex {
            field: **self,
            size: self.size(),
        }
    }
}

pub trait PlanarIndex<T: EmanationType>:
    Deref<Target = EField<Vec2<u32>, T>> + Send + Sync
{
    fn size(&self) -> Vec2<u32>;
    fn reduce(&self) -> ReducedIndex2d<T> {
        ReducedIndex2d {
            field: **self,
            size: self.size(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReducedIndex<T: EmanationType> {
    field: EField<u32, T>,
    size: u32,
}
impl<T: EmanationType> Deref for ReducedIndex<T> {
    type Target = EField<u32, T>;
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}
impl<T: EmanationType> ReducedIndex<T> {
    pub fn size(&self) -> u32 {
        self.size
    }
}
impl<T: EmanationType> LinearIndex<T> for ReducedIndex<T> {
    fn size(&self) -> u32 {
        self.size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReducedIndex2d<T: EmanationType> {
    field: EField<Vec2<u32>, T>,
    size: Vec2<u32>,
}
impl<T: EmanationType> Deref for ReducedIndex2d<T> {
    type Target = EField<Vec2<u32>, T>;
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}
impl<T: EmanationType> ReducedIndex2d<T> {
    pub fn size(&self) -> Vec2<u32> {
        self.size
    }
}
impl<T: EmanationType> PlanarIndex<T> for ReducedIndex2d<T> {
    fn size(&self) -> Vec2<u32> {
        self.size
    }
}

/// A field marking that a given [`Emanation<T>`] can be mapped to a sized one-dimensional array.
///
/// Also implements [`Domain`] via [`IndexDomain`], which allows [`Kernel`] calls over the array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayIndex<T: EmanationType> {
    field: EField<u32, T>,
    size: u32,
}
impl<T: EmanationType> ArrayIndex<T> {
    pub fn shrink(&self, size: u32) -> ArrayIndex<T> {
        ArrayIndex {
            field: self.field,
            size: self.size.min(size),
        }
    }
}
impl<T: EmanationType> Deref for ArrayIndex<T> {
    type Target = EField<u32, T>;
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<T: EmanationType> IndexEmanation<Expr<u32>> for ArrayIndex<T> {
    type T = T;
    fn bind_fields(&self, idx: Expr<u32>, element: &Element<T>) {
        element.bind(self.field, ValueAccessor(idx));
    }
}
impl<T: EmanationType> IndexDomain for ArrayIndex<T> {
    type I = Expr<u32>;
    type A = ();
    fn get_index(&self) -> Self::I {
        dispatch_id().x
    }
    fn dispatch_size(&self, _: ()) -> [u32; 3] {
        [self.size, 1, 1]
    }
}

impl<T: EmanationType> LinearIndex<T> for ArrayIndex<T> {
    fn size(&self) -> u32 {
        self.size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayIndex2d<T: EmanationType> {
    field: EField<Vec2<u32>, T>,
    size: Vec2<u32>,
}
impl<T: EmanationType> ArrayIndex2d<T> {
    pub fn shrink(&self, size: Vec2<u32>) -> ArrayIndex2d<T> {
        ArrayIndex2d {
            field: self.field,
            size: Vec2::new(self.size.x.min(size.x), self.size.y.min(size.y)),
        }
    }
}
impl<T: EmanationType> Deref for ArrayIndex2d<T> {
    type Target = EField<Vec2<u32>, T>;
    fn deref(&self) -> &Self::Target {
        &self.field
    }
}

impl<T: EmanationType> IndexEmanation<Expr<Vec2<u32>>> for ArrayIndex2d<T> {
    type T = T;
    fn bind_fields(&self, idx: Expr<Vec2<u32>>, element: &Element<T>) {
        element.bind(self.field, ValueAccessor(idx));
    }
}
impl<T: EmanationType> IndexDomain for ArrayIndex2d<T> {
    type I = Expr<Vec2<u32>>;
    type A = ();
    fn get_index(&self) -> Self::I {
        dispatch_id().xy()
    }
    fn dispatch_size(&self, _: ()) -> [u32; 3] {
        [self.size.x, self.size.y, 1]
    }
}

impl<T: EmanationType> PlanarIndex<T> for ArrayIndex2d<T> {
    fn size(&self) -> Vec2<u32> {
        self.size
    }
}

impl<T: EmanationType> ArrayIndex2d<T> {
    pub fn morton(&self, emanation: &Emanation<T>) -> ArrayIndex<T> {
        assert_eq!(
            self.size.x, self.size.y,
            "Morton indexing only supports square arrays."
        );
        assert!(
            self.size.x.is_power_of_two(),
            "Morton indexing only supports power-of-two arrays."
        );
        assert!(
            self.size.x <= 1 << 16,
            "Morton indexing only supports arrays with size < 65536."
        );
        let name = emanation.on(self.field).name() + "-morton";

        let field = self.field;
        let field = *emanation.create_field(&name).bind_fn(track!(move |el| {
            // https://graphics.stanford.edu/%7Eseander/bithacks.html#InterleaveBMN
            let index = field[[el]];
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
        }));
        ArrayIndex {
            field,
            size: self.size.x * self.size.y,
        }
    }
}

impl<T: EmanationType> Emanation<T> {
    pub fn create_index(&self, length: u32) -> ArrayIndex<T> {
        ArrayIndex {
            field: *self.create_field("index"),
            size: length,
        }
    }
    pub fn create_index2d(&self, size: Vec2<u32>) -> ArrayIndex2d<T> {
        ArrayIndex2d {
            field: *self.create_field("index2d"),
            size,
        }
    }
}
