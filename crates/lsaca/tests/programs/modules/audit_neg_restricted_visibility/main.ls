// audit: restricted visibility (pub(super), pub(self), pub(in path)) is not
// supported by the v1.7 language and is rejected while parsing.
mod helper;

fn main() {
    helper::value();
}
