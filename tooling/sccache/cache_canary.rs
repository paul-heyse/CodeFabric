pub fn codefabric_sccache_canary(value: u64) -> u64 {
    value.rotate_left(7) ^ 0x5a5a_5a5a_5a5a_5a5a
}
