pub fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

pub fn shuffle<T>(rows: &mut [T], state: &mut u64) {
    for i in (1..rows.len()).rev() {
        rows.swap(i, random(state) as usize % (i + 1));
    }
}
