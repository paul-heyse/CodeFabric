//! Application-owned operational and released-wire registry types.

include!("registry_contract_data.rs");

/// Closed failure returned when no retained operational state-machine edge matches an event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateTransitionViolation {
    pub prior_state: String,
    pub event: String,
    pub guard: String,
    pub error_code: &'static str,
}

/// Resolve one transition solely from an application-owned operational transition table.
///
/// # Errors
///
/// Returns `STATE_TRANSITION_VIOLATION` when the state, event, and proven guard do not
/// identify exactly one matching edge.
pub fn operational_transition(
    transitions: &'static [StateTransitionEntry],
    prior_state: &str,
    event: &str,
    guard: &str,
) -> Result<&'static StateTransitionEntry, StateTransitionViolation> {
    let mut matches = transitions.iter().filter(|transition| {
        transition.from == prior_state && transition.event == event && transition.guard == guard
    });
    let transition = matches.next().ok_or_else(|| StateTransitionViolation {
        prior_state: prior_state.to_owned(),
        event: event.to_owned(),
        guard: guard.to_owned(),
        error_code: "STATE_TRANSITION_VIOLATION",
    })?;
    if matches.next().is_some() {
        return Err(StateTransitionViolation {
            prior_state: prior_state.to_owned(),
            event: event.to_owned(),
            guard: guard.to_owned(),
            error_code: "STATE_TRANSITION_VIOLATION",
        });
    }
    Ok(transition)
}

/// Resolve one retained registry code to its canonical state name.
#[must_use]
pub fn registry_state_name(values: &[RegistryEntry], code: u16) -> Option<&'static str> {
    values
        .iter()
        .find(|entry| entry.code == code)
        .map(|entry| entry.name)
}

/// Resolve one public error solely from the released registry projection.
#[must_use]
pub fn public_error(name: &str) -> Option<&'static PublicErrorEntry> {
    PUBLIC_ERROR_ENTRIES.iter().find(|entry| entry.name == name)
}

/// Resolve a capability identifier to its append-only declaration-order code.
///
/// Capability registry entries are ordered authority records. AC-G-06 assigns registry codes in
/// declaration order starting at 10 and advancing by 10, so consumers never duplicate a second
/// capability allocation table.
#[must_use]
pub fn capability_code(name: &str) -> Option<u16> {
    CAPABILITY_IDS
        .iter()
        .zip(CAPABILITY_CODES)
        .find_map(|(candidate, code)| (*candidate == name).then_some(*code))
}
