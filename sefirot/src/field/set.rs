use std::collections::HashSet;

use pretty_type_name::pretty_type_name;

use super::*;

#[derive(Copy, Clone, PartialEq, Eq, Hash, UniqueId)]
pub struct FieldSetId {
    id: u64,
}
impl Debug for FieldSetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "S{}", self.id)
    }
}

pub struct FieldSet {
    pub(crate) id: FieldSetId,
    pub(crate) fields: HashSet<FieldHandle>,
}
impl Debug for FieldSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct FieldsWrapper<'a>(&'a HashSet<FieldHandle>);
        impl<'a> Debug for FieldsWrapper<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_map()
                    .entries(self.0.iter().map(|x| {
                        (
                            x,
                            x.field_desc()
                                .unwrap_or_else(|| "Field[dropped]".to_string()),
                        )
                    }))
                    .finish()
            }
        }
        f.debug_struct("FieldSet")
            .field("id", &self.id)
            .field("fields", &FieldsWrapper(&self.fields))
            .finish()
    }
}
impl FieldSet {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            id: FieldSetId::unique(),
            fields: HashSet::new(),
        }
    }
    pub fn id(&self) -> FieldSetId {
        self.id
    }
    pub fn create_field<X: Access, I: FieldIndex>(&self, name: impl AsRef<str>) -> Field<X, I> {
        let handle = FieldHandle::unique();
        FIELDS.insert(
            handle,
            RawField {
                name: name.as_ref().to_string(),
                access_type_name: pretty_type_name::<X>(),
                index_type_name: pretty_type_name::<I>(),
                binding: None,
            },
        );
        Field {
            handle,
            set: self.id,
            _marker: PhantomData,
        }
    }
}
