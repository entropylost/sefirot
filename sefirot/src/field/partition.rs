use crate::domain::{IndexDomain, IndexEmanation};
use crate::graph::{AddToComputeGraph, ComputeGraph, CopyFromBuffer};

use super::array::ArrayIndex;
use super::constant::ConstantAccessor;
use super::slice::Slice;
use super::*;
use tokio::sync::Mutex;

pub trait PartitionIndex: Value + Send + Sync {
    fn to(this: Self) -> u32;
    fn to_expr(this: Expr<Self>) -> Expr<u32>;
    fn null() -> Self;
}

impl PartitionIndex for u32 {
    fn to(this: Self) -> u32 {
        this
    }
    fn to_expr(this: Expr<Self>) -> Expr<u32> {
        this
    }
    fn null() -> Self {
        u32::MAX
    }
}

/// A set of fields that are used to partition an array, used as an argument to [`Emanation::partition`].
pub struct PartitionFields<I: PartitionIndex, T: EmanationType, P: EmanationType> {
    /// A field representing the partition index of an element, if this can be known at kernel build time.
    /// (eg: If using an [`ArrayPartitionDomain`])
    /// Should be unbound when passed in.
    pub const_partition: Field<I, T>,
    /// A field representing the partition index of an element.
    pub partition: EField<I, T>,
    pub partition_map: Field<Element<P>, T>,
}

#[derive(Debug, Clone)]
pub struct DynArrayPartitionDomain<I: PartitionIndex, T: EmanationType, P: EmanationType> {
    index: ArrayIndex<T>,
    partition: EField<I, T>,
    partition_ref: EField<u32, T>,
    partition_map: Field<Element<P>, T>,
    partition_lists: Field<Slice<Expr<u32>>, P>,
    sizes: Arc<Mutex<Vec<u32>>>,
    partition_index: ConstantAccessor<I, T>,
}
impl<I: PartitionIndex, T: EmanationType, P: EmanationType> IndexEmanation<Expr<u32>>
    for DynArrayPartitionDomain<I, T, P>
{
    type T = T;
    fn bind_fields(&self, index: Expr<u32>, element: &Element<Self::T>) {
        let partition_lists = self.partition_lists;
        let partition_map = self.partition_map;

        element.bind(self.partition, self.partition_index.clone());
        element.bind(self.partition_ref, ValueAccessor(index));
        element.bind(
            self.index.field,
            FnAccessor::new(move |el| {
                el.get(partition_map)
                    .unwrap()
                    .get(partition_lists)
                    .unwrap()
                    .read(index)
            }),
        );
    }
}
impl<I: PartitionIndex, T: EmanationType, P: EmanationType> IndexDomain
    for DynArrayPartitionDomain<I, T, P>
{
    type I = Expr<u32>;
    type A = I;
    fn get_index(&self) -> Self::I {
        dispatch_size().x
    }
    fn dispatch_size(&self, index: I) -> [u32; 3] {
        [self.sizes.blocking_lock()[I::to(index) as usize], 1, 1]
    }
    fn before_dispatch(&self, index: &I) {
        *self.partition_index.value.lock() = *index;
    }
}

#[derive(Debug, Clone)]
pub struct ArrayPartitionDomain<I: PartitionIndex, T: EmanationType, P: EmanationType> {
    index: ArrayIndex<T>,
    const_partition: Field<I, T>,
    partition: EField<I, T>,
    partition_ref: EField<u32, T>,
    partition_map: Field<Element<P>, T>,
    partition_lists: Field<Slice<Expr<u32>>, P>,
    sizes: Arc<Mutex<Vec<u32>>>,
    partition_index: I,
}
impl<I: PartitionIndex, T: EmanationType, P: EmanationType> IndexEmanation<Expr<u32>>
    for ArrayPartitionDomain<I, T, P>
{
    type T = T;
    fn bind_fields(&self, index: Expr<u32>, element: &Element<Self::T>) {
        let partition_index = self.partition_index;
        let partition_map = self.partition_map;
        let partition_lists = self.partition_lists;

        element.bind(self.const_partition, ValueAccessor(partition_index));
        element.bind(
            self.partition,
            FnAccessor::new(move |_| partition_index.expr()),
        );
        element.bind(self.partition_ref, ValueAccessor(index));
        element.bind(
            self.index.field,
            FnAccessor::new(move |el| {
                el.get(partition_map)
                    .unwrap()
                    .get(partition_lists)
                    .unwrap()
                    .read(index)
            }),
        );
    }
}
impl<I: PartitionIndex, T: EmanationType, P: EmanationType> IndexDomain
    for ArrayPartitionDomain<I, T, P>
{
    type I = Expr<u32>;
    type A = ();
    fn get_index(&self) -> Self::I {
        dispatch_size().x
    }
    fn dispatch_size(&self, _: ()) -> [u32; 3] {
        [
            self.sizes.blocking_lock()[I::to(self.partition_index) as usize],
            1,
            1,
        ]
    }
}
// TODO: Add support for other-Domain partitions using `IndexEmanation`.
#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
pub struct ArrayPartition<T: EmanationType, P: EmanationType, I: PartitionIndex> {
    index: ArrayIndex<T>,
    const_partition: Field<I, T>,
    partition: EField<I, T>,
    partition_ref: EField<u32, T>,
    partition_map: Field<Element<P>, T>,
    partition_lists: Field<Slice<Expr<u32>>, P>,
    partition_size: EField<u32, P>,
    partition_size_host: Arc<Mutex<Vec<u32>>>,
    update_lists_kernel: Kernel<T, fn()>,
    zero_lists_kernel: Kernel<P, fn()>,
}
impl<T: EmanationType, P: EmanationType, I: PartitionIndex> Debug for ArrayPartition<T, P, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "ArrayPartition<{}, {}> {{ .. }}",
            pretty_type_name::<T>(),
            pretty_type_name::<P>()
        ))
    }
}

impl<T: EmanationType, P: EmanationType, I: PartitionIndex> ArrayPartition<T, P, I> {
    /// The field representing the index of each element in a contiguous list of elements per partition.
    pub fn partition_ref(&self) -> EField<u32, T> {
        self.partition_ref
    }

    /// The field representing the size of each partition.
    pub fn partition_size(&self) -> EField<u32, P> {
        self.partition_size
    }

    /// The size of the partitions as a vector.
    /// May be out of date when the [`PartitionFields::partition`] field is changed, until the [`update`] function is called.
    pub fn partition_size_host(&self) -> Arc<Mutex<Vec<u32>>> {
        self.partition_size_host.clone()
    }

    /// Creates a domain for a partition with a kernel-constant index.
    pub fn select(&self, partition_index: I) -> ArrayPartitionDomain<I, T, P> {
        ArrayPartitionDomain {
            index: self.index,
            const_partition: self.const_partition,
            partition: self.partition,
            partition_ref: self.partition_ref,
            partition_map: self.partition_map,
            partition_lists: self.partition_lists,
            sizes: self.partition_size_host.clone(),
            partition_index,
        }
    }
    /// Creates a domain for a partition with an index that might vary between invocations.
    pub fn select_dyn(&self) -> DynArrayPartitionDomain<I, T, P> {
        DynArrayPartitionDomain {
            index: self.index,
            partition: self.partition,
            partition_ref: self.partition_ref,
            partition_map: self.partition_map,
            partition_lists: self.partition_lists,
            sizes: self.partition_size_host.clone(),
            partition_index: ConstantAccessor::new(I::null()),
        }
    }
}
impl<'b, T: EmanationType, P: EmanationType, I: PartitionIndex> CanReference
    for &'b ArrayPartition<T, P, I>
{
    type T = P;
}
impl<'a: 'b, 'b, T: EmanationType, P: EmanationType, I: PartitionIndex>
    Reference<'a, &'b ArrayPartition<T, P, I>>
{
    pub fn update<'c>(self) -> impl AddToComputeGraph<'c> + 'b {
        move |graph: &mut ComputeGraph<'c>| {
            let zero = *graph.add(self.zero_lists_kernel.dispatch());
            let update = *graph.add(self.update_lists_kernel.dispatch());
            let copy = *graph.add(CopyFromBuffer::new(
                &self.emanation.on(self.partition_size).buffer().unwrap(),
                self.partition_size_host.clone(),
            ));
            *graph.container().children_ordered(&[zero, update, copy])
        }
    }
}
impl<T: EmanationType> Emanation<T> {
    #[allow(clippy::double_parens)]
    pub fn partition<I: PartitionIndex, P: EmanationType>(
        &self,
        index: ArrayIndex<T>,
        partitions: &Emanation<P>,
        partition_index: ArrayIndex<P>,
        partition_fields: PartitionFields<I, T, P>,
        max_partition_size: Option<u32>,
    ) -> ArrayPartition<T, P, I> {
        let PartitionFields {
            const_partition,
            partition,
            partition_map,
        } = partition_fields;
        let max_partition_size = max_partition_size.unwrap_or(index.size);
        let partition_name = self.on(partition).name();
        let partition_ref = *self
            .create_field(&(partition_name.clone() + "-ref"))
            .bind_array(index, ());
        let partition_lists = *partitions
            .create_field(&(partition_name.clone() + "-lists"))
            .bind_array_slices(partition_index, max_partition_size, false, ());
        let partition_size = *partitions
            .create_field::<Expr<u32>>(&(partition_name.clone() + "-list-size"))
            .bind_array(partition_index, ());
        let partition_size_atomic = *partitions.on(partition_size).atomic();

        let update_lists_kernel = self.build_kernel::<fn()>(
            index,
            track!(&|el| {
                if I::to_expr(partition[[el]]) == I::to(I::null()) {
                    return;
                }
                let pt =
                    &partitions.get(&el.context, &partition_index, I::to_expr(partition[[el]]));
                let this_ref = partition_size_atomic[[pt]].fetch_add(1);
                partition_ref[[el]] = this_ref;
                partition_lists[[partition_map[[el]]]].write(this_ref, index.field[[el]]);
            }),
        );
        let zero_lists_kernel = partitions.build_kernel::<fn()>(
            partition_index,
            track!(&|el| {
                partition_size[[el]] = 0.expr();
            }),
        );
        ArrayPartition {
            index,
            const_partition,
            partition,
            partition_ref,
            partition_map,
            partition_lists,
            partition_size,
            partition_size_host: Arc::new(Mutex::new(vec![0; partition_index.size as usize])),
            update_lists_kernel,
            zero_lists_kernel,
        }
    }
}
