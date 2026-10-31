//! Base64 data URIs for the images embedded in an HTML file.

/// The RFC 4648 base64 alphabet.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes as standard padded base64.
pub(crate) fn base64(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    let (chunks, remainder) = bytes.as_chunks::<3>();
    for chunk in chunks {
        let [first, second, third] = *chunk;
        let value = (u32::from(first) << 16) | (u32::from(second) << 8) | u32::from(third);
        push_symbol(&mut output, value >> 18);
        push_symbol(&mut output, value >> 12);
        push_symbol(&mut output, value >> 6);
        push_symbol(&mut output, value);
    }
    match remainder {
        [first, second] => {
            let value = (u32::from(*first) << 16) | (u32::from(*second) << 8);
            push_symbol(&mut output, value >> 18);
            push_symbol(&mut output, value >> 12);
            push_symbol(&mut output, value >> 6);
            output.push('=');
        }
        [first] => {
            let value = u32::from(*first) << 16;
            push_symbol(&mut output, value >> 18);
            push_symbol(&mut output, value >> 12);
            output.push_str("==");
        }
        _ => {}
    }
    output
}

/// Builds a data URI from a media type and bytes.
pub(crate) fn data_uri(media_type: &str, bytes: &[u8]) -> String {
    format!("data:{media_type};base64,{}", base64(bytes))
}

fn push_symbol(output: &mut String, value: u32) {
    let index = usize::try_from(value & 0x3f).unwrap_or(0);
    match ALPHABET.get(index) {
        Some(symbol) => output.push(char::from(*symbol)),
        None => output.push('A'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_rfc_4648_vectors() {
        for (bytes, expected) in [
            (b"".as_slice(), ""),
            (b"f".as_slice(), "Zg=="),
            (b"fo".as_slice(), "Zm8="),
            (b"foo".as_slice(), "Zm9v"),
            (b"foob".as_slice(), "Zm9vYg=="),
            (b"fooba".as_slice(), "Zm9vYmE="),
            (b"foobar".as_slice(), "Zm9vYmFy"),
        ] {
            assert_eq!(base64(bytes), expected);
        }
    }

    #[test]
    fn a_data_uri_names_the_media_type_and_the_payload() {
        assert_eq!(data_uri("image/png", b"foo"), "data:image/png;base64,Zm9v");
    }
}
