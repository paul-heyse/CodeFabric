//! Operation-owned native row-visitor selectors. No process-global schema cache
//! is touched by the fallible path, and original allocations outlive all borrows.
use std::alloc::Layout;
use std::sync::Arc;
use crate::resource::{AllocationRequest,NativeResourceScope,ResourceExhausted};
use crate::{DeltaResult,Error};
use super::{ColumnName,ColumnNamesAndTypes,DataType,StructType};

/// The original owned names/types and their admission owners. Only borrowed
/// access is exposed: a caller cannot detach a bare Vec from its lifetime owner.
pub struct OwnedVisitorSelection {
    columns: ColumnNamesAndTypes,
    scope: Option<Arc<NativeResourceScope>>,
    #[cfg(feature="arrow-59")]
    arrow_owner: arrow_schema_59::resource::ResourceOwnerHandle,
}
impl OwnedVisitorSelection {
    pub fn as_ref(&self)->(&[ColumnName],&[DataType]) {self.columns.as_ref()}
    fn finish(columns:ColumnNamesAndTypes)->Self {
        Self{columns,scope:crate::resource::current_resource_scope(),
            #[cfg(feature="arrow-59")]
            arrow_owner:arrow_schema_59::resource::ResourceOwnerHandle::capture(),}
    }
    /// Copy an explicitly supplied original selector after its complete native
    /// layout is admitted. The source may be static; it is never initialized here.
    pub fn try_from_borrowed(names:&[ColumnName],types:&[DataType])->DeltaResult<Self>{
        let mut bytes=add(vec_bytes::<ColumnName>(names.len())?,vec_bytes::<DataType>(types.len())?)?;
        for name in names {bytes=add(bytes,vec_bytes::<String>(name.path().len())?)?;for part in name.path(){bytes=add(bytes,part.len())?;}}
        for kind in types {bytes=add(bytes,super::resource::type_copy_bytes(kind)?)?;}
        admit(bytes,"native_visitor_owned_selection")?;
        Ok(Self::finish((names.to_vec(),types.to_vec()).into()))
    }
    /// Construct from caller-borrowed path parts and original types. Stack
    /// literals need no LazyLock or intermediate ColumnName/String allocation.
    pub fn try_from_paths(names:&[&[&str]],types:&[DataType])->DeltaResult<Self>{
        if names.len()!=types.len(){return Err(pressure("native_visitor_selector_arity",names.len(),types.len()));}
        let mut bytes=add(vec_bytes::<ColumnName>(names.len())?,vec_bytes::<DataType>(types.len())?)?;
        for name in names {if name.len()>64{return Err(pressure("native_visitor_path_depth",name.len(),64));}bytes=add(bytes,vec_bytes::<String>(name.len())?)?;for part in *name{bytes=add(bytes,part.len())?;}}
        for kind in types {bytes=add(bytes,super::resource::type_copy_bytes(kind)?)?;}
        admit(bytes,"native_visitor_owned_selection")?;
        let names=names.iter().map(|parts|ColumnName::new(parts.iter().copied())).collect();
        Ok(Self::finish((names,types.to_vec()).into()))
    }
    /// Append an independently admitted selector without copying its strings or
    /// DataTypes. The common current operation owns both sides before transfer.
    pub(crate) fn try_append(mut self,other:Self)->DeltaResult<Self>{
        self.validate_current()?;other.validate_current()?;
        let (mut names,mut types)=self.columns.into_parts();
        let (other_names,other_types)=other.columns.into_parts();
        let length=add(names.len(),other_names.len())?;
        admit(add(vec_bytes::<ColumnName>(length)?,vec_bytes::<DataType>(length)?)?,"native_visitor_selection_append")?;
        names.try_reserve_exact(other_names.len()).map_err(|_|pressure("native_visitor_selection_append",length,0))?;
        types.try_reserve_exact(other_types.len()).map_err(|_|pressure("native_visitor_selection_append",length,0))?;
        names.extend(other_names);types.extend(other_types);self.columns=(names,types).into();Ok(self)
    }
    /// Run the original native GetSchemaLeaves traversal after checking its
    /// complete path/string/type and Vec growth from the original borrowed tree.
    pub fn try_from_schema(schema:&StructType, own_name:Option<&str>)->DeltaResult<Self>{
        if schema.resource_scope().is_some() && crate::resource::current_resource_scope().is_none(){return Err(pressure("native_visitor_schema_owner",1,0));}
        let mut shape=LeafShape{leaves:0,depth:usize::from(own_name.is_some()),bytes:own_name.map_or(0,str::len)};
        inspect(schema,usize::from(own_name.is_some()),own_name.map_or(0,str::len),&mut shape)?;
        let bytes=add(shape.bytes,add(growing_vec::<String>(shape.depth)?,add(growing_vec::<ColumnName>(shape.leaves)?,growing_vec::<DataType>(shape.leaves)?)?)?)?;
        admit(bytes,"native_visitor_schema_leaves")?;
        Ok(Self::finish(schema.leaves(own_name)))
    }
    /// Validate the original selector against current ownership before getters use it.
    pub(crate) fn validate_current(&self)->DeltaResult<()> {
        if let Some(scope)=&self.scope {scope.check_available()?;if crate::resource::current_resource_scope().is_none(){return Err(pressure("native_visitor_selection_owner",1,0));}}
        #[cfg(feature="arrow-59")]
        if !self.arrow_owner.is_empty() && arrow_schema_59::resource::current_resource_owner().is_none(){return Err(pressure("native_visitor_selection_arrow_owner",1,0));}
        Ok(())
    }
}
struct LeafShape {leaves:usize,depth:usize,bytes:usize}
fn inspect(schema:&StructType,depth:usize,prefix_bytes:usize,shape:&mut LeafShape)->DeltaResult<()> {
    if depth>64{return Err(pressure("native_visitor_path_depth",depth,64));}
    for field in schema.fields(){
        let depth=add(depth,1)?;if depth>64{return Err(pressure("native_visitor_path_depth",depth,64));}shape.depth=shape.depth.max(depth);
        let path_bytes=add(prefix_bytes,field.name.len())?;
        shape.bytes=add(shape.bytes,field.name.len())?; // Original traversal path push.
        if let DataType::Struct(inner)=field.data_type(){inspect(inner,depth,path_bytes,shape)?;}
        else{shape.leaves=add(shape.leaves,1)?;shape.bytes=add(shape.bytes,add(add(vec_bytes::<String>(depth)?,path_bytes)?,super::resource::type_copy_bytes(field.data_type())?)?)?;}
    }Ok(())
}
fn growing_vec<T>(count:usize)->DeltaResult<usize>{
    if count==0{return Ok(0);}
    let mut capacity=4usize;let mut bytes=vec_bytes::<T>(capacity)?;
    while capacity<count{capacity=capacity.checked_mul(2).ok_or_else(||pressure("native_visitor_vector_layout",usize::MAX,isize::MAX as usize))?;bytes=add(bytes,vec_bytes::<T>(capacity)?)?;}Ok(bytes)
}
fn vec_bytes<T>(count:usize)->DeltaResult<usize>{Layout::array::<T>(count).map(|layout|layout.size()).map_err(|_|pressure("native_visitor_vector_layout",count,isize::MAX as usize))}
fn add(a:usize,b:usize)->DeltaResult<usize>{a.checked_add(b).filter(|n|*n<=isize::MAX as usize).ok_or_else(||pressure("native_visitor_selection_layout",usize::MAX,isize::MAX as usize))}
fn pressure(kind:&'static str,requested:usize,limit:usize)->Error{ResourceExhausted{kind,requested,limit}.into()}
/// Whether a legacy selector must be rejected before calling its static initializer.
pub(crate) fn governed()->bool {
    if crate::resource::current_resource_scope().is_some(){return true;}
    #[cfg(feature="arrow-59")]
    if arrow_schema_59::resource::current_resource_owner().is_some(){return true;}
    false
}
pub(crate) fn admit(bytes:usize,kind:&'static str)->DeltaResult<()> {
    if bytes==0{return Ok(());}
    #[cfg(feature="arrow-59")]
    if let Some(owner)=arrow_schema_59::resource::current_resource_owner(){
        return owner.try_reserve_allocation(arrow_schema_59::resource::ResourceAllocationRequest{bytes,alignment:std::mem::align_of::<usize>(),kind}).map_err(|error|{owner.record_failure(error);pressure(error.kind,error.requested,error.limit)});
    }
    if let Some(scope)=crate::resource::current_resource_scope(){scope.reserve(AllocationRequest{bytes,kind})?;}
    Ok(())
}
pub(crate) fn legacy_selection(legacy:impl FnOnce()->(&'static[ColumnName],&'static[DataType]))->DeltaResult<OwnedVisitorSelection>{
    if governed(){return Err(pressure("native_visitor_owned_selection_required",1,0));}
    let (names,types)=legacy();OwnedVisitorSelection::try_from_borrowed(names,types)
}
