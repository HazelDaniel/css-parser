pub trait LocalError {
    type Out;

    fn resolve(&self) -> Self::Out;
}

#[rustfmt::skip]
#[derive(Debug)]
pub struct LexerSpan            (pub usize, pub usize);
