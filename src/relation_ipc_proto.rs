//! Shared construction and validation for the released relation-IPC protobuf envelope.
//!
//! The generated message types remain transport-only. This module is compiled verbatim into all
//! three Rust process domains and never serializes a semantic row outside Arrow IPC.

#![allow(dead_code)] // Producer and receiver domains intentionally use opposite halves.

use crate::relation_ipc_contract::{
    ARROW_IPC_METADATA_VERSION_V5, ARROW_TYPE_UNIVERSE, RELATION_IPC_FRAGMENT_BYTES,
    RELATION_IPC_PROTOCOL_VERSION, RelationWireIdentity, TYPED_RELATION_ENCODING,
};
use crate::relation_ipc_proto_types as wire;

const IPC_STREAM_EOS: [u8; 8] = [0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0];

/// One typed coverage remainder in the transport-neutral constructor input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RelationCoverageRemainder {
    pub(crate) scope: [u8; 16],
    pub(crate) unit_count: u64,
    pub(crate) reason: wire::RelationIpcRemainderReason,
}

/// Coverage state used to construct the required trailer and matching terminal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelationCoverage {
    pub(crate) status: wire::RelationIpcTerminalStatus,
    pub(crate) requested_units: u64,
    pub(crate) completed_units: u64,
    pub(crate) remainders: Vec<RelationCoverageRemainder>,
}

impl RelationCoverage {
    pub(crate) fn complete(requested_units: u64) -> Self {
        Self {
            status: wire::RelationIpcTerminalStatus::Complete,
            requested_units,
            completed_units: requested_units,
            remainders: Vec::new(),
        }
    }
}

/// Parsed header after every fixed-width and protocol-version invariant is checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParsedRelationHeader {
    pub(crate) identity: RelationWireIdentity,
    pub(crate) sequence: u64,
}

/// Parsed receiver-to-producer acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParsedRelationAck {
    pub(crate) header: ParsedRelationHeader,
    pub(crate) acknowledged_sequence: Option<u64>,
    pub(crate) released_bytes: u64,
    pub(crate) cancelled: bool,
}

/// Fragment an already encoded exact Arrow IPC stream into the released outer protocol.
///
/// Arrow bytes remain the only semantic carrier. Open, end, coverage, and terminal messages are
/// bounded typed control records. Producers must wait for a validated acknowledgement after each
/// returned payload frame before sending another payload beyond their granted credit window.
pub(crate) fn encode_relation_frames(
    identity: RelationWireIdentity,
    arrow_ipc: &[u8],
    declared_batches: u64,
    declared_rows: u64,
    coverage: &RelationCoverage,
) -> Result<Vec<wire::RelationIpcFrame>, &'static str> {
    if arrow_ipc.is_empty() || !arrow_ipc.ends_with(&IPC_STREAM_EOS) {
        return Err("relation Arrow IPC is empty or lacks its physical end marker");
    }
    validate_coverage(coverage)?;
    let mut sequence = 0_u64;
    let mut frames = Vec::with_capacity(
        4_usize.saturating_add(arrow_ipc.len().div_ceil(RELATION_IPC_FRAGMENT_BYTES)),
    );
    frames.push(wire::RelationIpcFrame {
        frame: Some(wire::relation_ipc_frame::Frame::Open(
            wire::RelationIpcOpen {
                header: Some(wire_header(identity, sequence)),
                requested_units: coverage.requested_units,
                arrow_type_universe: ARROW_TYPE_UNIVERSE.to_owned(),
                metadata_version: ARROW_IPC_METADATA_VERSION_V5,
                semantic_encoding: TYPED_RELATION_ENCODING.to_owned(),
            },
        )),
    });
    sequence = next_sequence(sequence)?;
    for fragment in arrow_ipc.chunks(RELATION_IPC_FRAGMENT_BYTES) {
        frames.push(wire::RelationIpcFrame {
            frame: Some(wire::relation_ipc_frame::Frame::Payload(
                wire::RelationIpcPayload {
                    header: Some(wire_header(identity, sequence)),
                    arrow_ipc_fragment: fragment.to_vec(),
                },
            )),
        });
        sequence = next_sequence(sequence)?;
    }
    frames.push(wire::RelationIpcFrame {
        frame: Some(wire::relation_ipc_frame::Frame::IpcEnd(
            wire::RelationIpcEnd {
                header: Some(wire_header(identity, sequence)),
                declared_ipc_bytes: u64::try_from(arrow_ipc.len()).unwrap_or(u64::MAX),
                declared_batches,
                declared_rows,
            },
        )),
    });
    sequence = next_sequence(sequence)?;
    frames.push(wire::RelationIpcFrame {
        frame: Some(wire::relation_ipc_frame::Frame::CoverageTrailer(
            wire::RelationIpcCoverageTrailer {
                header: Some(wire_header(identity, sequence)),
                status: coverage.status as i32,
                requested_units: coverage.requested_units,
                completed_units: coverage.completed_units,
                remainders: coverage
                    .remainders
                    .iter()
                    .map(|remainder| wire::RelationIpcCoverageRemainder {
                        scope: remainder.scope.to_vec(),
                        unit_count: remainder.unit_count,
                        reason: remainder.reason as i32,
                    })
                    .collect(),
            },
        )),
    });
    sequence = next_sequence(sequence)?;
    frames.push(wire::RelationIpcFrame {
        frame: Some(wire::relation_ipc_frame::Frame::Terminal(
            wire::RelationIpcTerminal {
                header: Some(wire_header(identity, sequence)),
                status: coverage.status as i32,
            },
        )),
    });
    Ok(frames)
}

/// Build one acknowledgement frame in its independent per-stream sequence space.
pub(crate) fn flow_control_ack_frame(
    identity: RelationWireIdentity,
    ack_sequence: u64,
    acknowledged_sequence: Option<u64>,
    released_bytes: u64,
    cancelled: bool,
) -> Result<wire::RelationIpcFrame, &'static str> {
    validate_ack_shape(acknowledged_sequence, released_bytes, cancelled)?;
    Ok(wire::RelationIpcFrame {
        frame: Some(wire::relation_ipc_frame::Frame::FlowControlAck(
            wire::RelationIpcFlowControlAck {
                header: Some(wire_header(identity, ack_sequence)),
                acknowledged_sequence,
                released_bytes,
                cancelled,
            },
        )),
    })
}

/// Decode an acknowledgement and reject every data-direction or malformed frame.
pub(crate) fn decode_flow_control_ack(
    frame: &wire::RelationIpcFrame,
) -> Result<ParsedRelationAck, &'static str> {
    let wire::relation_ipc_frame::Frame::FlowControlAck(ack) = frame
        .frame
        .as_ref()
        .ok_or("relation IPC frame variant is absent")?
    else {
        return Err("data-direction relation frame appeared on the acknowledgement stream");
    };
    let header = parse_header(ack.header.as_ref())?;
    validate_ack_shape(ack.acknowledged_sequence, ack.released_bytes, ack.cancelled)?;
    Ok(ParsedRelationAck {
        header,
        acknowledged_sequence: ack.acknowledged_sequence,
        released_bytes: ack.released_bytes,
        cancelled: ack.cancelled,
    })
}

pub(crate) fn parse_header(
    header: Option<&wire::RelationIpcFrameHeader>,
) -> Result<ParsedRelationHeader, &'static str> {
    let header = header.ok_or("relation IPC frame header is absent")?;
    if header.protocol_version != u32::from(RELATION_IPC_PROTOCOL_VERSION) {
        return Err("relation IPC protocol version differs");
    }
    let identity = parse_identity(
        header
            .identity
            .as_ref()
            .ok_or("relation IPC stream identity is absent")?,
    )?;
    Ok(ParsedRelationHeader {
        identity,
        sequence: header.sequence,
    })
}

pub(crate) fn validate_open_profile(open: &wire::RelationIpcOpen) -> Result<(), &'static str> {
    if open.requested_units == 0
        || open.arrow_type_universe != ARROW_TYPE_UNIVERSE
        || open.metadata_version != ARROW_IPC_METADATA_VERSION_V5
        || open.semantic_encoding != TYPED_RELATION_ENCODING
    {
        return Err("relation IPC open profile, requested units, or Arrow universe differs");
    }
    Ok(())
}

fn wire_header(identity: RelationWireIdentity, sequence: u64) -> wire::RelationIpcFrameHeader {
    wire::RelationIpcFrameHeader {
        protocol_version: u32::from(RELATION_IPC_PROTOCOL_VERSION),
        identity: Some(wire::RelationIpcStreamIdentity {
            relation_id: identity.relation_id.to_vec(),
            stream_id: identity.stream_id.to_vec(),
            schema_fingerprint: identity.schema_fingerprint.to_vec(),
            source_pin: identity.source_pin.to_vec(),
            context_pin: identity.context_pin.to_vec(),
        }),
        sequence,
    }
}

fn parse_identity(
    identity: &wire::RelationIpcStreamIdentity,
) -> Result<RelationWireIdentity, &'static str> {
    Ok(RelationWireIdentity {
        relation_id: fixed(&identity.relation_id, "relation identity is not 16 bytes")?,
        stream_id: fixed(&identity.stream_id, "stream identity is not 16 bytes")?,
        schema_fingerprint: fixed(
            &identity.schema_fingerprint,
            "schema fingerprint is not 32 bytes",
        )?,
        source_pin: fixed(&identity.source_pin, "source pin is not 32 bytes")?,
        context_pin: fixed(&identity.context_pin, "context pin is not 32 bytes")?,
    })
}

fn fixed<const N: usize>(bytes: &[u8], message: &'static str) -> Result<[u8; N], &'static str> {
    let value: [u8; N] = bytes.try_into().map_err(|_| message)?;
    if value == [0; N] {
        return Err("relation IPC identity component is zero");
    }
    Ok(value)
}

fn validate_ack_shape(
    acknowledged_sequence: Option<u64>,
    released_bytes: u64,
    cancelled: bool,
) -> Result<(), &'static str> {
    if (cancelled && (acknowledged_sequence.is_some() || released_bytes != 0))
        || (!cancelled && (acknowledged_sequence.is_none() || released_bytes == 0))
    {
        return Err("relation IPC acknowledgement shape is invalid");
    }
    Ok(())
}

fn validate_coverage(coverage: &RelationCoverage) -> Result<(), &'static str> {
    if coverage.requested_units == 0 || coverage.completed_units > coverage.requested_units {
        return Err("relation coverage unit bounds are invalid");
    }
    let mut scopes = std::collections::BTreeSet::new();
    let mut remainder_units = 0_u64;
    let mut has_unknown = false;
    for remainder in &coverage.remainders {
        if remainder.scope == [0; 16]
            || remainder.unit_count == 0
            || !scopes.insert(remainder.scope)
            || remainder.reason == wire::RelationIpcRemainderReason::Unspecified
        {
            return Err("relation coverage remainder is invalid");
        }
        remainder_units = remainder_units
            .checked_add(remainder.unit_count)
            .ok_or("relation coverage remainder count overflowed")?;
        has_unknown |= remainder.reason == wire::RelationIpcRemainderReason::Unknown;
    }
    if coverage
        .completed_units
        .checked_add(remainder_units)
        .ok_or("relation coverage count overflowed")?
        != coverage.requested_units
    {
        return Err("relation coverage does not close the request");
    }
    let valid = match coverage.status {
        wire::RelationIpcTerminalStatus::Complete => {
            coverage.completed_units == coverage.requested_units && coverage.remainders.is_empty()
        }
        wire::RelationIpcTerminalStatus::Partial => {
            coverage.completed_units < coverage.requested_units
                && !coverage.remainders.is_empty()
                && !has_unknown
        }
        wire::RelationIpcTerminalStatus::Unknown => {
            coverage.completed_units < coverage.requested_units
                && !coverage.remainders.is_empty()
                && has_unknown
        }
        wire::RelationIpcTerminalStatus::Unspecified => false,
    };
    valid
        .then_some(())
        .ok_or("relation coverage status and remainder accounting differ")
}

fn next_sequence(sequence: u64) -> Result<u64, &'static str> {
    sequence
        .checked_add(1)
        .ok_or("relation IPC sequence space is exhausted")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> RelationWireIdentity {
        RelationWireIdentity {
            relation_id: [1; 16],
            stream_id: [2; 16],
            schema_fingerprint: [3; 32],
            source_pin: [4; 32],
            context_pin: [5; 32],
        }
    }

    #[test]
    fn frames_preserve_exact_open_profile_and_terminal_order() {
        let mut ipc = vec![9; RELATION_IPC_FRAGMENT_BYTES + 1];
        ipc.extend_from_slice(&IPC_STREAM_EOS);
        let frames =
            encode_relation_frames(identity(), &ipc, 1, 7, &RelationCoverage::complete(1)).unwrap();

        assert_eq!(frames.len(), 6);
        let wire::relation_ipc_frame::Frame::Open(open) = frames[0].frame.as_ref().unwrap() else {
            panic!("first frame is not open")
        };
        validate_open_profile(open).unwrap();
        assert_eq!(parse_header(open.header.as_ref()).unwrap().sequence, 0);
        assert!(matches!(
            frames.last().unwrap().frame,
            Some(wire::relation_ipc_frame::Frame::Terminal(_))
        ));
    }

    #[test]
    fn ack_direction_widths_and_cancellation_fail_closed() {
        let accepted = flow_control_ack_frame(identity(), 0, Some(1), 32, false).unwrap();
        assert_eq!(
            decode_flow_control_ack(&accepted).unwrap(),
            ParsedRelationAck {
                header: ParsedRelationHeader {
                    identity: identity(),
                    sequence: 0,
                },
                acknowledged_sequence: Some(1),
                released_bytes: 32,
                cancelled: false,
            }
        );
        assert!(flow_control_ack_frame(identity(), 1, Some(1), 1, true).is_err());

        let mut malformed = accepted;
        if let Some(wire::relation_ipc_frame::Frame::FlowControlAck(ack)) = &mut malformed.frame {
            ack.header
                .as_mut()
                .unwrap()
                .identity
                .as_mut()
                .unwrap()
                .stream_id
                .pop();
        }
        assert!(decode_flow_control_ack(&malformed).is_err());
    }
}
