use std::collections::HashSet;

use super::*;

// TODO: Remove?
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
    pub(crate) prefix: String,
}
impl Debug for FieldSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct FieldsWrapper<'a>(&'a HashSet<FieldHandle>);
        impl<'a> Debug for FieldsWrapper<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_map()
                    .entries(self.0.iter().map(|x| {
                        (
                            x.0,
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
            prefix: "".to_string(),
        }
    }
    pub fn prefix(self, prefix: impl AsRef<str>) -> Self {
        Self {
            prefix: prefix.as_ref().to_string(),
            ..self
        }
    }
    pub fn get_prefix(&self) -> &str {
        &self.prefix
    }
    pub fn id(&self) -> FieldSetId {
        self.id
    }
    pub fn create<X: Access, I: FieldIndex>(&mut self, name: impl AsRef<str>) -> Field<X, I> {
        let (field, handle) = Field::create(name);
        self.fields.insert(handle);
        field
    }
    pub fn create_bind<X: Access, I: FieldIndex>(
        &mut self,
        name: impl AsRef<str>,
        mapping: impl Mapping<X, I> + Send + Sync,
    ) -> Field<X, I> {
        self.create::<X, I>(self.prefix.clone() + name.as_ref())
            .bind(mapping)
    }
}
