use std::io::{self, Read};

pub(super) struct PipeCapture {
    bytes: Vec<u8>,
    observed: usize,
}

impl PipeCapture {
    pub(super) fn into_parts(self) -> (Vec<u8>, usize) {
        (self.bytes, self.observed)
    }
}

/// Retains at most `limit` bytes while continuing to drain the pipe to EOF.
pub(super) fn read_pipe(mut pipe: impl Read, limit: usize) -> io::Result<PipeCapture> {
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    let mut observed = 0usize;
    let mut buffer = [0u8; 8 * 1024];
    loop {
        let count = pipe.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        observed = observed.saturating_add(count);
        let retained = limit.saturating_sub(bytes.len()).min(count);
        bytes.extend_from_slice(&buffer[..retained]);
    }
    Ok(PipeCapture { bytes, observed })
}

#[cfg(test)]
mod tests {
    use super::read_pipe;
    use std::io::Cursor;

    #[test]
    fn retains_binary_bytes_at_and_below_the_limit() {
        let bytes = [0, 1, 0xff, 3];
        let capture = read_pipe(Cursor::new(bytes), bytes.len()).unwrap();
        let (captured, observed) = capture.into_parts();
        assert_eq!(captured, bytes);
        assert_eq!(observed, bytes.len());
    }

    #[test]
    fn drains_beyond_the_limit_without_retaining_the_suffix() {
        let bytes = (0..=255).cycle().take(32 * 1024).collect::<Vec<_>>();
        let capture = read_pipe(Cursor::new(&bytes), 257).unwrap();
        let (captured, observed) = capture.into_parts();
        assert_eq!(captured, bytes[..257]);
        assert_eq!(observed, bytes.len());
    }
}
