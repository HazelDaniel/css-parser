pub trait LocalError {
    type Out;

    fn resolve(&self) -> Self::Out;
}

#[rustfmt::skip]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexerSpan            (pub usize, pub usize);
