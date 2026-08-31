//! Application-owned conversion between released protobuf frames and relation-IPC domain types.

use arrow_schema::SchemaRef;

use crate::relation_ipc::{
    ContextPin, CoverageRemainder, CoverageScope, CoverageTrailer, CoverageTrailerFrame,
    FlowControlAck, FrameHeader, IpcEndOfStream, RelationId, RelationIpcFrame, RelationPayload,
    RelationStreamContract, RelationTerminal, RemainderReason, SchemaFingerprint, SourcePin,
    StreamId, StreamIdentity, StreamOpen, TerminalStatus,
};
use crate::relation_ipc_contract::{RelationWireIdentity, relation_wire_identity};
use crate::relation_ipc_proto::{
    ParsedRelationHeader, decode_flow_control_ack, flow_control_ack_frame, parse_header,
    validate_open_profile,
};
use crate::rpc::generated::codefabric::provider::v1 as wire;

/// Derive the contract the daemon expects independently from provider-supplied frame identities.
pub(crate) fn relation_stream_contract(
    relation_name: &str,
    schema: SchemaRef,
    provider_run_id: &str,
    scope_id: &str,
    source_digest: &str,
    context_digest: &str,
    requested_units: u64,
) -> Result<RelationStreamContract, String> {
    let schema_digest = schema
        .metadata()
        .get("codefabric.schema_digest")
        .ok_or_else(|| "application-owned relation schema digest is absent".to_owned())?;
    let identity = relation_wire_identity(
        relation_name,
        schema_digest,
        provider_run_id,
        scope_id,
        source_digest,
        context_digest,
    )
    .map_err(str::to_owned)?;
    Ok(RelationStreamContract {
        identity: domain_identity(identity),
        schema,
        requested_units,
    })
}

/// Decode one transport frame without admitting it. Sequence, schema, Arrow, coverage, credit,
/// and terminal validation remain the responsibility of [`crate::relation_ipc::RelationIpcAssembler`].
pub(crate) fn decode_relation_frame(
    frame: wire::RelationIpcFrame,
) -> Result<RelationIpcFrame, String> {
    use wire::relation_ipc_frame::Frame;
    match frame
        .frame
        .ok_or_else(|| "relation IPC frame variant is absent".to_owned())?
    {
        Frame::Open(open) => {
            validate_open_profile(&open).map_err(str::to_owned)?;
            Ok(RelationIpcFrame::Open(StreamOpen {
                header: domain_header(parse_header(open.header.as_ref()).map_err(str::to_owned)?),
                requested_units: open.requested_units,
            }))
        }
        Frame::Payload(payload) => Ok(RelationIpcFrame::Payload(RelationPayload {
            header: domain_header(parse_header(payload.header.as_ref()).map_err(str::to_owned)?),
            payload: payload.arrow_ipc_fragment,
        })),
        Frame::FlowControlAck(wire_ack) => {
            let ack = decode_flow_control_ack(&wire::RelationIpcFrame {
                frame: Some(Frame::FlowControlAck(wire_ack)),
            })
            .map_err(str::to_owned)?;
            Ok(RelationIpcFrame::FlowControlAck(FlowControlAck {
                header: domain_header(ack.header),
                acknowledged_sequence: ack.acknowledged_sequence,
                released_bytes: ack.released_bytes,
                cancelled: ack.cancelled,
            }))
        }
        Frame::IpcEnd(end) => Ok(RelationIpcFrame::IpcEnd(IpcEndOfStream {
            header: domain_header(parse_header(end.header.as_ref()).map_err(str::to_owned)?),
            declared_ipc_bytes: end.declared_ipc_bytes,
            declared_batches: end.declared_batches,
            declared_rows: end.declared_rows,
        })),
        Frame::CoverageTrailer(trailer) => {
            let status = terminal_status(trailer.status)?;
            let remainders = trailer
                .remainders
                .into_iter()
                .map(|remainder| {
                    Ok(CoverageRemainder {
                        scope: CoverageScope(fixed::<16>(
                            &remainder.scope,
                            "coverage remainder scope is not 16 bytes",
                        )?),
                        unit_count: remainder.unit_count,
                        reason: remainder_reason(remainder.reason)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(RelationIpcFrame::CoverageTrailer(CoverageTrailerFrame {
                header: domain_header(
                    parse_header(trailer.header.as_ref()).map_err(str::to_owned)?,
                ),
                trailer: CoverageTrailer {
                    status,
                    requested_units: trailer.requested_units,
                    completed_units: trailer.completed_units,
                    remainders,
                },
            }))
        }
        Frame::Terminal(terminal) => Ok(RelationIpcFrame::Terminal(RelationTerminal {
            header: domain_header(parse_header(terminal.header.as_ref()).map_err(str::to_owned)?),
            status: terminal_status(terminal.status)?,
        })),
    }
}

/// Encode one domain frame for protocol fixtures and receiver acknowledgements.
pub(crate) fn encode_relation_frame(
    frame: &RelationIpcFrame,
) -> Result<wire::RelationIpcFrame, String> {
    use wire::relation_ipc_frame::Frame;
    let frame = match frame {
        RelationIpcFrame::Open(open) => Frame::Open(wire::RelationIpcOpen {
            header: Some(wire_header(open.header)),
            requested_units: open.requested_units,
            arrow_type_universe: crate::relation_ipc_contract::ARROW_TYPE_UNIVERSE.to_owned(),
            metadata_version: crate::relation_ipc_contract::ARROW_IPC_METADATA_VERSION_V5,
            semantic_encoding: crate::relation_ipc_contract::TYPED_RELATION_ENCODING.to_owned(),
        }),
        RelationIpcFrame::Payload(payload) => Frame::Payload(wire::RelationIpcPayload {
            header: Some(wire_header(payload.header)),
            arrow_ipc_fragment: payload.payload.clone(),
        }),
        RelationIpcFrame::FlowControlAck(ack) => {
            return flow_control_ack_frame(
                wire_identity(ack.header.identity),
                ack.header.sequence,
                ack.acknowledged_sequence,
                ack.released_bytes,
                ack.cancelled,
            )
            .map_err(str::to_owned);
        }
        RelationIpcFrame::IpcEnd(end) => Frame::IpcEnd(wire::RelationIpcEnd {
            header: Some(wire_header(end.header)),
            declared_ipc_bytes: end.declared_ipc_bytes,
            declared_batches: end.declared_batches,
            declared_rows: end.declared_rows,
        }),
        RelationIpcFrame::CoverageTrailer(trailer) => {
            Frame::CoverageTrailer(wire::RelationIpcCoverageTrailer {
                header: Some(wire_header(trailer.header)),
                status: wire_terminal_status(trailer.trailer.status) as i32,
                requested_units: trailer.trailer.requested_units,
                completed_units: trailer.trailer.completed_units,
                remainders: trailer
                    .trailer
                    .remainders
                    .iter()
                    .map(|remainder| wire::RelationIpcCoverageRemainder {
                        scope: remainder.scope.0.to_vec(),
                        unit_count: remainder.unit_count,
                        reason: wire_remainder_reason(remainder.reason) as i32,
                    })
                    .collect(),
            })
        }
        RelationIpcFrame::Terminal(terminal) => Frame::Terminal(wire::RelationIpcTerminal {
            header: Some(wire_header(terminal.header)),
            status: wire_terminal_status(terminal.status) as i32,
        }),
    };
    Ok(wire::RelationIpcFrame { frame: Some(frame) })
}

fn domain_header(header: ParsedRelationHeader) -> FrameHeader {
    FrameHeader {
        protocol_version: crate::relation_ipc::RELATION_IPC_PROTOCOL_VERSION,
        identity: domain_identity(header.identity),
        sequence: header.sequence,
    }
}

fn domain_identity(identity: RelationWireIdentity) -> StreamIdentity {
    StreamIdentity {
        relation_id: RelationId(identity.relation_id),
        stream_id: StreamId(identity.stream_id),
        schema_fingerprint: SchemaFingerprint(identity.schema_fingerprint),
        source_pin: SourcePin(identity.source_pin),
        context_pin: ContextPin(identity.context_pin),
    }
}

fn wire_header(header: FrameHeader) -> wire::RelationIpcFrameHeader {
    let identity = wire_identity(header.identity);
    wire::RelationIpcFrameHeader {
        protocol_version: u32::from(header.protocol_version),
        identity: Some(wire::RelationIpcStreamIdentity {
            relation_id: identity.relation_id.to_vec(),
            stream_id: identity.stream_id.to_vec(),
            schema_fingerprint: identity.schema_fingerprint.to_vec(),
            source_pin: identity.source_pin.to_vec(),
            context_pin: identity.context_pin.to_vec(),
        }),
        sequence: header.sequence,
    }
}

fn wire_identity(identity: StreamIdentity) -> RelationWireIdentity {
    RelationWireIdentity {
        relation_id: identity.relation_id.0,
        stream_id: identity.stream_id.0,
        schema_fingerprint: identity.schema_fingerprint.0,
        source_pin: identity.source_pin.0,
        context_pin: identity.context_pin.0,
    }
}

fn terminal_status(value: i32) -> Result<TerminalStatus, String> {
    match wire::RelationIpcTerminalStatus::try_from(value) {
        Ok(wire::RelationIpcTerminalStatus::Complete) => Ok(TerminalStatus::Complete),
        Ok(wire::RelationIpcTerminalStatus::Partial) => Ok(TerminalStatus::Partial),
        Ok(wire::RelationIpcTerminalStatus::Unknown) => Ok(TerminalStatus::Unknown),
        Ok(wire::RelationIpcTerminalStatus::Unspecified) | Err(_) => {
            Err("relation IPC terminal status is unspecified or unregistered".to_owned())
        }
    }
}

fn remainder_reason(value: i32) -> Result<RemainderReason, String> {
    match wire::RelationIpcRemainderReason::try_from(value) {
        Ok(wire::RelationIpcRemainderReason::Unsupported) => Ok(RemainderReason::Unsupported),
        Ok(wire::RelationIpcRemainderReason::ProviderUnavailable) => {
            Ok(RemainderReason::ProviderUnavailable)
        }
        Ok(wire::RelationIpcRemainderReason::ResourceLimit) => Ok(RemainderReason::ResourceLimit),
        Ok(wire::RelationIpcRemainderReason::InvalidSource) => Ok(RemainderReason::InvalidSource),
        Ok(wire::RelationIpcRemainderReason::Cancelled) => Ok(RemainderReason::Cancelled),
        Ok(wire::RelationIpcRemainderReason::Unknown) => Ok(RemainderReason::Unknown),
        Ok(wire::RelationIpcRemainderReason::Unspecified) | Err(_) => {
            Err("relation IPC remainder reason is unspecified or unregistered".to_owned())
        }
    }
}

const fn wire_terminal_status(status: TerminalStatus) -> wire::RelationIpcTerminalStatus {
    match status {
        TerminalStatus::Complete => wire::RelationIpcTerminalStatus::Complete,
        TerminalStatus::Partial => wire::RelationIpcTerminalStatus::Partial,
        TerminalStatus::Unknown => wire::RelationIpcTerminalStatus::Unknown,
    }
}

const fn wire_remainder_reason(reason: RemainderReason) -> wire::RelationIpcRemainderReason {
    match reason {
        RemainderReason::Unsupported => wire::RelationIpcRemainderReason::Unsupported,
        RemainderReason::ProviderUnavailable => {
            wire::RelationIpcRemainderReason::ProviderUnavailable
        }
        RemainderReason::ResourceLimit => wire::RelationIpcRemainderReason::ResourceLimit,
        RemainderReason::InvalidSource => wire::RelationIpcRemainderReason::InvalidSource,
        RemainderReason::Cancelled => wire::RelationIpcRemainderReason::Cancelled,
        RemainderReason::Unknown => wire::RelationIpcRemainderReason::Unknown,
    }
}

fn fixed<const N: usize>(bytes: &[u8], message: &str) -> Result<[u8; N], String> {
    let value: [u8; N] = bytes.try_into().map_err(|_| message.to_owned())?;
    if value == [0; N] {
        return Err("relation IPC fixed identity is zero".to_owned());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use prost::Message as _;

    use super::*;

    fn identity() -> StreamIdentity {
        StreamIdentity {
            relation_id: RelationId([1; 16]),
            stream_id: StreamId([2; 16]),
            schema_fingerprint: SchemaFingerprint([3; 32]),
            source_pin: SourcePin([4; 32]),
            context_pin: ContextPin([5; 32]),
        }
    }

    #[test]
    fn every_domain_variant_round_trips_through_binary_protobuf() {
        let header = FrameHeader::current(identity(), 7);
        let frames = vec![
            RelationIpcFrame::Open(StreamOpen {
                header,
                requested_units: 1,
            }),
            RelationIpcFrame::Payload(RelationPayload {
                header,
                payload: vec![1, 2, 3],
            }),
            RelationIpcFrame::FlowControlAck(FlowControlAck {
                header,
                acknowledged_sequence: Some(1),
                released_bytes: 3,
                cancelled: false,
            }),
            RelationIpcFrame::IpcEnd(IpcEndOfStream {
                header,
                declared_ipc_bytes: 3,
                declared_batches: 1,
                declared_rows: 2,
            }),
            RelationIpcFrame::CoverageTrailer(CoverageTrailerFrame {
                header,
                trailer: CoverageTrailer::complete(1),
            }),
            RelationIpcFrame::Terminal(RelationTerminal {
                header,
                status: TerminalStatus::Complete,
            }),
        ];
        for frame in frames {
            let encoded = encode_relation_frame(&frame).unwrap().encode_to_vec();
            let decoded = wire::RelationIpcFrame::decode(encoded.as_slice()).unwrap();
            assert_eq!(decode_relation_frame(decoded).unwrap(), frame);
        }
    }

    #[test]
    fn wrong_arrow_universe_and_identity_width_are_rejected_before_payload() {
        let domain = RelationIpcFrame::Open(StreamOpen {
            header: FrameHeader::current(identity(), 0),
            requested_units: 1,
        });
        let mut wrong_universe = encode_relation_frame(&domain).unwrap();
        if let Some(wire::relation_ipc_frame::Frame::Open(open)) = &mut wrong_universe.frame {
            open.arrow_type_universe = "arrow-array@60".to_owned();
        }
        assert!(decode_relation_frame(wrong_universe).is_err());

        let mut wrong_width = encode_relation_frame(&domain).unwrap();
        if let Some(wire::relation_ipc_frame::Frame::Open(open)) = &mut wrong_width.frame {
            open.header
                .as_mut()
                .unwrap()
                .identity
                .as_mut()
                .unwrap()
                .source_pin
                .pop();
        }
        assert!(decode_relation_frame(wrong_width).is_err());
    }
}
