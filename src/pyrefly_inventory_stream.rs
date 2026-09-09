//! Ordered, bounded descriptor upload shared by the daemon and isolated Pyrefly server.

use super::{AnalyzeCommand, AnalyzeModulesRequest, Command, ModuleRequest};
use prost::Message as _;
use tokio_stream::{Stream, StreamExt as _};
use tonic::Status;

pub(crate) const MAX_MODULES_PER_RUN: usize = 16_384;
pub(crate) const MODULES_PER_CHUNK: usize = 64;
pub(crate) const MAX_SOURCE_BYTES_PER_MODULE: u64 = 32 * 1024 * 1024;
pub(crate) const MAX_SOURCE_BYTES_PER_RUN: u64 = 512 * 1024 * 1024;
const MAX_DESCRIPTOR_BYTES: usize = 32 * 1024 * 1024;

/// Before the complete upload is validated, no caller may mutate checker state or accept a run.
#[allow(
    dead_code,
    clippy::too_many_lines,
    reason = "Shared ordered upload validation runs in the server and in daemon wire tests"
)]
pub(crate) async fn receive_inventory<S>(
    start: &mut AnalyzeModulesRequest,
    commands: &mut S,
) -> Result<(), Status>
where
    S: Stream<Item = Result<AnalyzeCommand, Status>> + Unpin,
{
    let Some(expected) = start.expected_module_count else {
        if start.modules.len() > MODULES_PER_CHUNK
            || start.changed_module_ids.len() > MODULES_PER_CHUNK
        {
            return Err(Status::resource_exhausted(
                "large inventories require descriptor chunks",
            ));
        }
        return Ok(());
    };
    if expected as usize > MAX_MODULES_PER_RUN
        || !start.modules.is_empty()
        || !start.changed_module_ids.is_empty()
    {
        return Err(Status::invalid_argument("invalid chunked inventory start"));
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Status::internal("system clock predates epoch"))?
        .as_millis();
    let remaining = u128::try_from(start.deadline_unix_ms)
        .ok()
        .and_then(|end| end.checked_sub(now))
        .and_then(|ms| u64::try_from(ms).ok())
        .filter(|ms| *ms > 0)
        .ok_or_else(|| Status::deadline_exceeded("inventory upload deadline reached"))?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(remaining);
    let mut chunks = 0_u32;
    let mut bytes = 0_usize;
    let mut modules = Vec::<ModuleRequest>::new();
    let mut changed = Vec::new();
    loop {
        let command = tokio::time::timeout_at(deadline, commands.next())
            .await
            .map_err(|_| Status::deadline_exceeded("inventory upload deadline reached"))?
            .ok_or_else(|| {
                Status::invalid_argument("inventory upload ended without terminal counts")
            })??;
        bytes = bytes
            .checked_add(command.encoded_len())
            .filter(|bytes| *bytes <= MAX_DESCRIPTOR_BYTES)
            .ok_or_else(|| {
                Status::resource_exhausted("inventory descriptors exceed the aggregate byte bound")
            })?;
        match command.command {
            Some(Command::InventoryChunk(chunk)) => {
                if chunk.sequence != chunks
                    || chunks as usize >= 2 * MAX_MODULES_PER_RUN / MODULES_PER_CHUNK
                    || chunk.modules.len() > MODULES_PER_CHUNK
                    || chunk.changed_module_ids.len() > MODULES_PER_CHUNK
                    || (chunk.modules.is_empty() && chunk.changed_module_ids.is_empty())
                    || modules.len() + chunk.modules.len() > expected as usize
                    || changed.len() + chunk.changed_module_ids.len() > MAX_MODULES_PER_RUN
                {
                    return Err(Status::invalid_argument(
                        "inventory chunk sequence or extent differs",
                    ));
                }
                modules.extend(chunk.modules);
                changed.extend(chunk.changed_module_ids);
                chunks += 1;
            }
            Some(Command::InventoryEnd(end)) => {
                if end.chunk_count != chunks
                    || end.module_count != expected
                    || modules.len() != expected as usize
                    || end.changed_module_count as usize != changed.len()
                {
                    return Err(Status::invalid_argument("inventory terminal counts differ"));
                }
                // Commit the assembled descriptor set only after a complete end marker.
                start.modules = modules;
                start.changed_module_ids = changed;
                return Ok(());
            }
            Some(Command::Cancel(cancel))
                if cancel.provider_run_id == start.provider_run_id && !cancel.reason.is_empty() =>
            {
                return Err(Status::cancelled("inventory upload cancelled"));
            }
            _ => {
                return Err(Status::invalid_argument(
                    "unexpected command during inventory upload",
                ));
            }
        }
    }
}
