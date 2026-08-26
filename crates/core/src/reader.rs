use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReaderError {
    Io {
        kind: std::io::ErrorKind,
        message: String,
    },
    InvalidUtf8 {
        valid_up_to: usize,
        error_len: Option<usize>,
    },
}

pub struct Reader<R: Read> {
    source: Option<R>,
    cached: Option<Result<String, ReaderError>>,
}

impl<R: Read> Reader<R> {
    pub fn new(source: R) -> Self {
        Self {
            source: Some(source),
            cached: None,
        }
    }

    pub fn read(&mut self) -> Result<&str, ReaderError> {
        if self.cached.is_none() {
            let mut bytes = Vec::new();
            let result = match self.source.take() {
                Some(mut source) => match source.read_to_end(&mut bytes) {
                    Ok(_) => String::from_utf8(bytes).map_err(|error| ReaderError::InvalidUtf8 {
                        valid_up_to: error.utf8_error().valid_up_to(),
                        error_len: error.utf8_error().error_len(),
                    }),
                    Err(error) => Err(ReaderError::Io {
                        kind: error.kind(),
                        message: error.to_string(),
                    }),
                },
                None => unreachable!("reader source is consumed after the first read"),
            };

            self.cached = Some(result);
        }

        match self.cached.as_ref().expect("reader result was just cached") {
            Ok(source) => Ok(source.as_str()),
            Err(error) => Err(error.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reader_decodes_utf8_input() {
        let mut reader = Reader::new(Cursor::new("body { color: é; }".as_bytes()));

        assert_eq!(reader.read().unwrap(), "body { color: é; }");
    }

    #[test]
    fn reader_caches_successful_reads() {
        let mut reader = Reader::new(Cursor::new(b"cached"));

        assert_eq!(reader.read().unwrap(), "cached");
        assert_eq!(reader.read().unwrap(), "cached");
    }

    #[test]
    fn reader_reports_invalid_utf8() {
        let mut reader = Reader::new(Cursor::new(vec![b'a', 0xFF, b'b']));

        assert_eq!(
            reader.read().unwrap_err(),
            ReaderError::InvalidUtf8 {
                valid_up_to: 1,
                error_len: Some(1),
            }
        );
    }

    #[test]
    fn reader_caches_decoding_errors() {
        let mut reader = Reader::new(Cursor::new(vec![0xFF]));

        let first = reader.read().unwrap_err();
        let second = reader.read().unwrap_err();
        assert_eq!(first, second);
    }
}
