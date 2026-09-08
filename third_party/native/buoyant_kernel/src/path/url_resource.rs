//! Explicit native URI admission. The selected local profile performs an
//! allocation-free origin check before url 2.5.8 can enter arbitrary IDNA input.
use std::{cell::RefCell, marker::PhantomData, rc::Rc, sync::Arc};
use url::Url;
use crate::{DeltaResult, Error, OwnedFileMeta};
use crate::resource::{AllocationRequest, NativeResourceScope, ResourceExhausted};
use crate::crc::resource::{add, mul};

/// Deployment-owned origin authorization and preallocation before native join.
/// Implementations reserve complete original parser layouts on `scope`; the
/// native parser remains responsible for URI resolution and normalization.
pub trait NativeUrlJoinAdmission: Send + Sync + std::fmt::Debug {
    fn preflight(&self, scope: &NativeResourceScope, base: &Url, reference: &str) -> DeltaResult<()>;
    fn validate_joined(&self, scope: &NativeResourceScope, joined: &Url) -> DeltaResult<()>;
}

#[derive(Clone, Debug)]
pub struct NativeUrlThreadPolicy {
    admission: Arc<dyn NativeUrlJoinAdmission>,
    scope: Arc<NativeResourceScope>,
}
thread_local! { static URL_POLICY: RefCell<Option<NativeUrlThreadPolicy>> = const { RefCell::new(None) }; }
pub struct NativeUrlThreadGuard {
    previous: Option<NativeUrlThreadPolicy>,
    _not_send: PhantomData<Rc<()>>,
}
impl Drop for NativeUrlThreadGuard {
    fn drop(&mut self) { URL_POLICY.with(|slot| *slot.borrow_mut() = self.previous.take()); }
}
impl NativeUrlThreadPolicy {
    pub fn new(scope: Arc<NativeResourceScope>, admission: Arc<dyn NativeUrlJoinAdmission>) -> Self { Self { admission, scope } }
    pub fn enter_thread(&self) -> DeltaResult<NativeUrlThreadGuard> {
        if !crate::resource::current_resource_scope().as_ref().is_some_and(|current| Arc::ptr_eq(current, &self.scope))
            || URL_POLICY.with(|slot| slot.borrow().as_ref().is_some_and(|policy| !Arc::ptr_eq(&policy.scope, &self.scope))) {
            return Err(ResourceExhausted { kind: "native_url_foreign_scope", requested: 1, limit: 0 }.into());
        }
        let previous = URL_POLICY.with(|slot| slot.replace(Some(self.clone())));
        Ok(NativeUrlThreadGuard { previous, _not_send: PhantomData })
    }
}

fn policy(scope: &Arc<NativeResourceScope>) -> DeltaResult<NativeUrlThreadPolicy> {
    URL_POLICY.with(|slot| match slot.borrow().as_ref() {
        Some(policy) if Arc::ptr_eq(scope, &policy.scope) => Ok(policy.clone()),
        _ => Err(ResourceExhausted { kind: "native_url_join_policy_missing", requested: 1, limit: 0 }.into()),
    })
}

impl OwnedFileMeta {
    /// Join the actual original reference only after explicit policy admission.
    /// The returned URL remains inseparable from the original allocation scope.
    pub fn try_from_reference(base: &Url, reference: &str, last_modified: i64, size: u64) -> DeltaResult<Self> {
        let scope = crate::resource::current_resource_scope();
        if let Some(scope) = &scope { scope.check_available()?; }
        let admission = scope.as_ref().map(policy).transpose()?;
        if let Some(admission) = &admission {
            admission.admission.preflight(&admission.scope, base, reference)?;
        }
        let location = base.join(reference)?;
        if let Some(admission) = &admission {
            admission.admission.validate_joined(&admission.scope, &location)?;
        }
        Ok(Self::from_admitted_parts(crate::FileMeta { location, last_modified, size }, scope))
    }
}

#[derive(Debug)]
struct DeniedLocalUrl { reason: &'static str, _scope: Arc<NativeResourceScope> }
impl std::fmt::Display for DeniedLocalUrl { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.reason) } }
impl std::error::Error for DeniedLocalUrl {}
fn denied(scope: &Arc<NativeResourceScope>, reason: &'static str) -> DeltaResult<()> {
    scope.reserve(AllocationRequest { kind: "native_local_url_diagnostic", bytes: std::mem::size_of::<DeniedLocalUrl>() })?;
    Err(Error::generic_err(DeniedLocalUrl { reason, _scope: scope.clone() }))
}

/// An explicit local deployment profile, configured from authorized directory
/// URLs. Physical descriptor/no-symlink authorization still belongs to the owned
/// ObjectStore; this callback authorizes URI origin and canonical URL roots.
#[derive(Debug)]
pub struct LocalUrlJoinAdmission {
    roots: Vec<Url>,
    max_reference_bytes: usize,
    _owner: Arc<NativeResourceScope>,
}
impl LocalUrlJoinAdmission {
    pub fn try_new(scope: Arc<NativeResourceScope>, roots: &[Url], max_reference_bytes: usize) -> DeltaResult<Arc<Self>> {
        if max_reference_bytes == 0 || roots.is_empty() || roots.iter().any(|root| !is_local(root) || !root.path().ends_with('/')) {
            return Err(ResourceExhausted { kind: "native_local_url_profile", requested: 1, limit: 0 }.into());
        }
        let mut bytes = add(std::mem::size_of::<Self>(), 2 * std::mem::size_of::<usize>())?;
        bytes = add(bytes, mul(roots.len(), std::mem::size_of::<Url>())?)?;
        for root in roots { bytes = add(bytes, root.as_str().len())?; }
        scope.reserve(AllocationRequest { kind: "native_local_url_profile", bytes })?;
        Ok(Arc::new(Self { roots: roots.to_vec(), max_reference_bytes, _owner: scope }))
    }
    fn contains(&self, url: &Url) -> bool {
        is_local(url) && self.roots.iter().any(|root| url.path().starts_with(root.path()))
    }
}
fn is_local(url: &Url) -> bool { url.scheme() == "file" && url.host_str().is_none() && url.query().is_none() && url.fragment().is_none() }
fn normalized(input: &str) -> impl Iterator<Item = char> + Clone + '_ {
    input.trim_matches(|c: char| c <= '\u{20}').chars().filter(|c| !matches!(c, '\t' | '\n' | '\r'))
}
fn local_reference(input: &str) -> bool {
    let mut chars = normalized(input);
    let mut scheme = chars.clone();
    let first = scheme.next();
    if first.is_some_and(|c| c.is_ascii_alphabetic()) {
        let mut length = 0;
        let mut is_file = true;
        let mut scan = chars.clone();
        while let Some(c) = scan.next() {
            if c == ':' {
                if !is_file || length != 4 { return false; }
                chars = scan;
                break;
            }
            if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) { break; }
            is_file &= "file".as_bytes().get(length).is_some_and(|expected| c.eq_ignore_ascii_case(&(*expected as char)));
            length += 1;
        }
    }
    let mut authority = chars.clone();
    if matches!(authority.next(), Some('/' | '\\')) && matches!(authority.next(), Some('/' | '\\')) {
        let mut host = authority.take_while(|c| !matches!(c, '/' | '\\' | '?' | '#'));
        let mut count = 0;
        let mut localhost = true;
        while let Some(c) = host.next() {
            let c = if c == '%' {
                let Some(high) = host.next().and_then(|c| c.to_digit(16)) else { return false; };
                let Some(low) = host.next().and_then(|c| c.to_digit(16)) else { return false; };
                char::from((high * 16 + low) as u8)
            } else { c };
            localhost &= "localhost".as_bytes().get(count).is_some_and(|expected| c.eq_ignore_ascii_case(&(*expected as char)));
            count += 1;
        }
        return count == 0 || (count == 9 && localhost);
    }
    true
}
impl NativeUrlJoinAdmission for LocalUrlJoinAdmission {
    fn preflight(&self, scope: &NativeResourceScope, base: &Url, reference: &str) -> DeltaResult<()> {
        // Request the current scope's Arc only for retained diagnostics; never
        // reserve new allocations against the profile's older retained owner.
        let owner = crate::resource::current_resource_scope().ok_or(ResourceExhausted { kind: "native_url_scope_missing", requested: 1, limit: 0 })?;
        if reference.len() > self.max_reference_bytes {
            return Err(ResourceExhausted { kind: "native_url_reference_bytes", requested: reference.len(), limit: self.max_reference_bytes }.into());
        }
        if !self.contains(base) || !local_reference(reference) { return denied(&owner, "URL reference is outside the configured local origin"); }
        // url parser serialization starts at reference.len. Scheme scanning may
        // fill then clear it. File/relative branches copy <=base.len and path,
        // query, fragment encoding emits <=3 bytes per original UTF-8 byte.
        // parse_path(file) split_off owns a simultaneous full path copy. Four
        // times the maximum serialization includes cumulative geometric String
        // layouts; an additional maximum path copy and ignored-char host String
        // coexist. Localhost's sole ASCII9 label stays inside IDNA's inline
        // [char;253]/[AlreadyAsciiLabel;8] buffers; only <=9 output bytes allocate.
        let output = add(add(base.as_str().len(), mul(reference.len(), 3)?)?, 16)?;
        let bytes = add(mul(output, 6)?, add(mul(add(reference.len(), 8)?, 4)?, 4 * 16)?)?;
        scope.reserve(AllocationRequest { kind: "native_local_url_join", bytes: add(bytes, std::mem::size_of::<OwnedFileMeta>())? })
    }
    fn validate_joined(&self, _scope: &NativeResourceScope, joined: &Url) -> DeltaResult<()> {
        if self.contains(joined) { return Ok(()); }
        let owner = crate::resource::current_resource_scope().ok_or(ResourceExhausted { kind: "native_url_scope_missing", requested: 1, limit: 0 })?;
        denied(&owner, "Resolved URL is outside the configured local roots")
    }
}
