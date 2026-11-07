//! Explicit reader policy for backup producer versions, separate from container format.

pub fn reader_accepts_producer(reader: &str, producer: &str) -> bool {
    reader == producer || matches!((reader, producer), ("0.0.2", "0.0.1"))
}

pub fn current_reader_accepts(producer: &str) -> bool {
    reader_accepts_producer(env!("CARGO_PKG_VERSION"), producer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_forward_compatibility_is_allowed() {
        assert!(reader_accepts_producer("0.0.1", "0.0.1"));
        assert!(reader_accepts_producer("0.0.2", "0.0.1"));
        assert!(reader_accepts_producer("0.0.2", "0.0.2"));
        assert!(!reader_accepts_producer("0.0.1", "0.0.2"));
        assert!(!reader_accepts_producer("0.0.2", "0.0.0"));
        assert!(!reader_accepts_producer("0.0.2", "0.0.3"));
        assert!(!reader_accepts_producer("0.0.3", "0.0.1"));
        assert!(!reader_accepts_producer("0.0.2", "0.0.1-dev"));
    }
}
