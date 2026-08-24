pub trait LocalError {
    type Out;

    fn resolve (&self) -> Self::Out;
}
