use crate::domain::{IndexDomain, IndexEmanation};
use crate::graph::{AddToComputeGraph, ComputeGraph, CopyFromBuffer};

use super::array::ArrayIndex;
use super::constant::ConstantAccessor;
use super::slice::Slice;
use super::*;
use luisa::lang::types::AtomicRef;
use tokio::sync::Mutex;

pub const NULL_PARTITION: u32 = u32::MAX;

#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
pub struct ArrayPartition<T: EmanationType, P: EmanationType> {
    pub index: ArrayIndex<T>,
    pub partition_index: ArrayIndex<P>,
    pub const_partition: Field<u32, T>,
    pub partition: Field<Expr<u32>, T>,
    pub partition_ref: Field<Expr<u32>, T>,
    #[allow(dead_code)]
    partition_lists: Field<Slice<Expr<u32>>, T>,
    partition_size: Field<Expr<u32>, P>,
    #[allow(dead_code)]
    partition_size_atomic: Field<AtomicRef<u32>, P>,
    partition_size_host: Arc<Mutex<Vec<u32>>>,
    update_lists_kernel: Kernel<T, fn()>,
    zero_lists_kernel: Kernel<P, fn()>,
}

#[derive(Debug, Clone)]
pub struct DynArrayPartitionDomain<T: EmanationType> {
    partition: Field<Expr<u32>, T>,
    partition_ref: Field<Expr<u32>, T>,
    sizes: Arc<Mutex<Vec<u32>>>,
    index: ConstantAccessor<u32, T>,
}
impl<T: EmanationType> IndexEmanation<Expr<u32>> for DynArrayPartitionDomain<T> {
    type T = T;
    fn bind_fields(&self, index: Expr<u32>, element: &Element<Self::T>) {
        element.bind(self.partition, self.index.clone());

        element.bind(self.partition_ref, ValueAccessor(index));
    }
}
impl<T: EmanationType> IndexDomain for DynArrayPartitionDomain<T> {
    type I = Expr<u32>;
    type A = u32;
    fn get_index(&self) -> Self::I {
        dispatch_size().x
    }
    fn dispatch_size(&self, index: u32) -> [u32; 3] {
        [self.sizes.blocking_lock()[index as usize], 1, 1]
    }
}

#[derive(Debug, Clone)]
pub struct ArrayPartitionDomain<T: EmanationType> {
    const_partition: Field<u32, T>,
    partition: Field<Expr<u32>, T>,
    partition_ref: Field<Expr<u32>, T>,
    sizes: Arc<Mutex<Vec<u32>>>,
    index: u32,
}
impl<T: EmanationType> IndexEmanation<Expr<u32>> for ArrayPartitionDomain<T> {
    type T = T;
    fn bind_fields(&self, index: Expr<u32>, element: &Element<Self::T>) {
        let partition_index = self.index;
        element.bind(self.const_partition, ValueAccessor(partition_index));
        element.bind(
            self.partition,
            FnAccessor::new(move |_| partition_index.expr()),
        );

        element.bind(self.partition_ref, ValueAccessor(index));
    }
}
impl<T: EmanationType> IndexDomain for ArrayPartitionDomain<T> {
    type I = Expr<u32>;
    type A = ();
    fn get_index(&self) -> Self::I {
        dispatch_size().x
    }
    fn dispatch_size(&self, _: ()) -> [u32; 3] {
        [self.sizes.blocking_lock()[self.index as usize], 1, 1]
    }
}
impl<T: EmanationType, P: EmanationType> ArrayPartition<T, P> {
    /// Creates a domain for a partition with a kernel-constant index.
    pub fn select(&self, index: u32) -> ArrayPartitionDomain<T> {
        ArrayPartitionDomain {
            const_partition: self.const_partition,
            partition: self.partition,
            partition_ref: self.partition_ref,
            sizes: self.partition_size_host.clone(),
            index,
        }
    }
    /// Creates a domain for a partition with an index that might vary between invocations.
    pub fn select_dyn(&self) -> DynArrayPartitionDomain<T> {
        DynArrayPartitionDomain {
            partition: self.partition,
            partition_ref: self.partition_ref,
            sizes: self.partition_size_host.clone(),
            index: ConstantAccessor::new(0),
        }
    }
}
impl<'b, T: EmanationType, P: EmanationType> CanReference for &'b ArrayPartition<T, P> {
    type T = P;
}
impl<'a: 'b, 'b, T: EmanationType, P: EmanationType> Reference<'a, &'b ArrayPartition<T, P>> {
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
    pub fn partition<P: EmanationType>(
        &self,
        index: ArrayIndex<T>,
        partitions: &Emanation<P>,
        partition_index: ArrayIndex<P>,
        partition: Field<Expr<u32>, T>,
        max_partition_size: Option<u32>,
    ) -> ArrayPartition<T, P> {
        let max_partition_size = max_partition_size.unwrap_or(index.size);
        let partition_name = self.on(partition).name();
        let partition_ref = *self
            .create_field(&(partition_name.clone() + "-ref"))
            .bind_array(index, ());
        let partition_lists = *self
            .create_field(&(partition_name.clone() + "-lists"))
            .bind_array_slices(index, max_partition_size, false, ());
        let partition_size = *partitions
            .create_field(&(partition_name.clone() + "-list-size"))
            .bind_array(partition_index, ());
        let partition_size_atomic = *partitions.on(partition_size).atomic();
        ArrayPartition {
            index,
            partition_index,
            const_partition: *self.create_field(&(partition_name.clone() + "-const")),
            partition,
            partition_ref,
            partition_lists,
            partition_size,
            partition_size_atomic,
            partition_size_host: Arc::new(Mutex::new(vec![0; partition_index.size as usize])),
            update_lists_kernel: self.build_kernel::<fn()>(
                index,
                track!(&|el| {
                    if partition[[el]] == NULL_PARTITION {
                        return;
                    }
                    let pt = &partitions.get(&el.context, &partition_index, partition[[el]]);
                    let this_ref = partition_size_atomic[[pt]].fetch_add(1);
                    partition_ref[[el]] = this_ref;
                    partition_lists[[el]].write(this_ref, index.field[[el]]);
                }),
            ),
            zero_lists_kernel: partitions.build_kernel::<fn()>(
                partition_index,
                track!(&|el| {
                    partition_size[[el]] = 0.expr();
                }),
            ),
        }
    }
}
