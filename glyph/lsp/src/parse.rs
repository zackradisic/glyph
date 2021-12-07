use combine::{
    from_str,
    parser::{
        combinator::{any_send_partial_state, AnySendPartialState},
        range::{range, take, take_while1},
    },
    skip_many, ParseError, Parser, RangeStream,
};

pub fn decode_header<'a, I>(
) -> impl Parser<I, Output = Vec<u8>, PartialState = AnySendPartialState> + 'a
where
    I: RangeStream<Token = u8, Range = &'a [u8]> + 'a,
    // Necessary due to rust-lang/rust#24159
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    let content_length =
        range(&b"Content-Length: "[..]).with(from_str(take_while1(|b: u8| b.is_ascii_digit())));

    any_send_partial_state(
        (
            skip_many(range(&b"\r\n"[..])),
            content_length,
            range(&b"\r\n\r\n"[..]).map(|_| ()),
        )
            .then_partial(|&mut (_, message_length, _)| {
                take(message_length).map(|bytes: &[u8]| bytes.to_owned())
            }),
    )
}
