use crate::domain::IndexEmanation;
use crate::graph::{AddToComputeGraph, ComputeGraph, CopyFromBuffer};

use super::array::ArrayIndex;
use super::*;
use luisa::lang::types::AtomicRef;
use luisa::prelude::tracked;
use tokio::sync::Mutex;

#[cfg_attr(
    feature = "bevy",
    derive(bevy_ecs::prelude::Resource, bevy_ecs::prelude::Component)
)]
#[derive(Clone)]
pub struct BufferSlice<V: Value> {
    pub buffer: Buffer<V>,
    pub offset: Expr<u32>,
    pub size: Expr<u32>,
    pub check_bounds: bool,
}
impl<V: Value> BufferSlice<V> {
    #[tracked]
    pub fn read(&self, index: Expr<u32>) -> Expr<V> {
        if self.check_bounds {
            let i = index < self.size;
            lc_assert!(i);
        }
        self.buffer.read(index + self.offset)
    }
    #[tracked]
    pub fn write(&self, index: Expr<u32>, value: Expr<V>) {
        if self.check_bounds {
            let i = index < self.size;
            lc_assert!(i);
        }
        self.buffer.write(index + self.offset, value)
    }
}

pub struct BufferSlicesAccessor<V: Value, T: EmanationType> {
    pub index: ArrayIndex<T>,
    pub buffer: Buffer<V>,
    pub slice_size: u32,
    pub check_bounds: bool,
}
impl<V: Value, T: EmanationType> Accessor<T> for BufferSlicesAccessor<V, T> {
    type V = BufferSlice<V>;
    type C = BufferSlice<V>;

    #[tracked]
    fn get(&self, element: &Element<T>, field: Field<Self::V, T>) -> Result<Self::V, ReadError> {
        Ok(self
            .get_or_insert_cache(element, field, || BufferSlice {
                buffer: self.buffer.clone(),
                offset: element.get(self.index.field).unwrap() * self.slice_size,
                size: self.slice_size.expr(),
                check_bounds: self.check_bounds,
            })
            .clone())
    }
    fn set(
        &self,
        _element: &Element<T>,
        _field: Field<Self::V, T>,
        _value: &Self::V,
    ) -> Result<(), WriteError> {
        Err(WriteError {
            message: "Cannot write to `BufferSlice` field.".to_string(),
        })
    }

    fn save(&self, _element: &Element<T>, _field: Field<Self::V, T>) {
        unreachable!();
    }

    fn can_write(&self) -> bool {
        false
    }
}

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
    partition_lists: Field<BufferSlice<u32>, T>,
    partition_size: Field<Expr<u32>, P>,
    #[allow(dead_code)]
    partition_size_atomic: Field<AtomicRef<u32>, P>,
    partition_size_host: Arc<Mutex<Vec<u32>>>,
    update_lists_kernel: Kernel<T, fn()>,
    zero_lists_kernel: Kernel<P, fn()>,
}
pub struct ArrayPartitionDomain<T: EmanationType> {
    const_partition: Field<u32, T>,
    partition: Field<Expr<u32>, T>,
    partition_ref: Field<Expr<u32>, T>,
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
impl<T: EmanationType, P: EmanationType> ArrayPartition<T, P> {
    pub fn index(&self, index: u32) -> ArrayPartitionDomain<T> {
        ArrayPartitionDomain {
            const_partition: self.const_partition,
            partition: self.partition,
            partition_ref: self.partition_ref,
            index,
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
            **graph.container().children_ordered(&[zero, update, copy])
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
            .bind(BufferSlicesAccessor {
                index,
                buffer: self
                    .device
                    .create_buffer((max_partition_size * partition_index.size) as usize),
                slice_size: max_partition_size,
                check_bounds: false,
            });
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
            update_lists_kernel: self.build_kernel::<fn()>(index, &|el| {
                let p = &partitions.get(&el.context, &partition_index, *partition.at(el));
                let this_ref = (*partition_size_atomic.at(p)).fetch_add(1);
                *partition_ref.at(el) = this_ref;
                partition_lists.at(el).write(this_ref, *index.field.at(el));
            }),
            zero_lists_kernel: partitions.build_kernel::<fn()>(partition_index, &|el| {
                *partition_size.at(el) = 0.expr();
            }),
        }
    }
}
