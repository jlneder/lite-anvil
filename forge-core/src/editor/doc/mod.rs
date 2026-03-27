#[allow(clippy::module_inception)] // doc::doc maps to doc_native, renaming would obscure that
pub(crate) mod doc;
pub(crate) mod doc_layout;
pub(crate) mod doc_module;
pub(crate) mod doc_search;
pub(crate) mod doc_translate;
pub(crate) mod docview;
pub(crate) mod symbol_index;
