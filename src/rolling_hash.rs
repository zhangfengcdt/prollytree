use std::hash::Hash;
use std::hash::Hasher;
use twox_hash::XxHash64;

const BASE: u64 = 257;
const MOD: u64 = 1_000_000_007;

pub trait RollingHash<const N: usize> {
    fn calculate_initial_hash(
        &self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        window_size: usize,
    ) -> u64;
    fn rolling_hash(&self, keys: &[Vec<u8>], values: &[Vec<u8>], window_size: usize) -> Vec<u64>;
}

pub struct DefaultRollingHash<const N: usize>;

impl Default for DefaultRollingHash<32> {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultRollingHash<32> {
    pub fn new() -> Self {
        DefaultRollingHash
    }

    fn polynomial_hash<T: Hash>(data: &[T], base: u64, modulus: u64) -> u64 {
        let mut hash = 0;
        for item in data {
            let mut hasher = XxHash64::with_seed(0);
            item.hash(&mut hasher);
            hash = (hash * base + hasher.finish()) % modulus;
        }
        hash
    }
}

impl<const N: usize> RollingHash<N> for DefaultRollingHash<N> {
    fn calculate_initial_hash(
        &self,
        keys: &[Vec<u8>],
        values: &[Vec<u8>],
        window_size: usize,
    ) -> u64 {
        let keys_hash = DefaultRollingHash::polynomial_hash(&keys[..window_size], BASE, MOD);
        let values_hash = DefaultRollingHash::polynomial_hash(&values[..window_size], BASE, MOD);
        (keys_hash + values_hash) % MOD
    }

    fn rolling_hash(&self, keys: &[Vec<u8>], values: &[Vec<u8>], window_size: usize) -> Vec<u64> {
        let mut result = Vec::new();
        if keys.len() < window_size || values.len() < window_size {
            return result;
        }

        let mut current_hash = self.calculate_initial_hash(keys, values, window_size);
        result.push(current_hash);

        let base_power = (0..window_size).fold(1, |acc, _| (acc * BASE) % MOD);

        for i in 1..=(keys.len() - window_size) {
            for &byte in &keys[i - 1] {
                current_hash = (current_hash + MOD - byte as u64 * base_power % MOD) % MOD;
            }
            for &byte in &values[i - 1] {
                current_hash = (current_hash + MOD - byte as u64 * base_power % MOD) % MOD;
            }

            current_hash = (current_hash * BASE) % MOD;

            for &byte in &keys[i + window_size - 1] {
                current_hash = (current_hash + byte as u64) % MOD;
            }
            for &byte in &values[i + window_size - 1] {
                current_hash = (current_hash + byte as u64) % MOD;
            }

            result.push(current_hash);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_hash() {
        let mut keys = Vec::new();
        let mut values = Vec::new();

        for i in 0..20 {
            keys.push(vec![i * 3 + 1, i * 3 + 2, i * 3 + 3]);
            values.push(vec![i * 3 + 61, i * 3 + 62, i * 3 + 63]);
        }

        let rolling_hash = DefaultRollingHash::<2>;

        let rolling_hashes = rolling_hash.rolling_hash(&keys, &values, 2);

        // The expected hashes would be calculated based on the given implementation
        // Here, we just check if we get a vector of the right length for simplicity
        assert_eq!(rolling_hashes.len(), 19);

        // Check if the hash values are as expected
        // These values need to be precomputed or known to be correct
        let expected_hashes = vec![
            468763316, 213049765, 189124964, 734908500, 695733616, 322245842, 30345940, 706529002,
            180032062, 564776778, 138625441, 312189946, 612724767, 544630569, 738879153, 355222235,
            449852346, 464248049, 858401043,
        ];
        assert_eq!(rolling_hashes, expected_hashes);
    }
}
