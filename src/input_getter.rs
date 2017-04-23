use std::io::BufRead;
use std::process;

/// Reads a line from from `reader` and strips trailing whitespace.
pub fn get_string<T: BufRead>(reader: &mut T) -> Result<String, String> {
    let mut input = String::new();
    match reader.read_line(&mut input) {
        Ok(character_count) if character_count == 0 => {
            // A signal was sent - just exit the process as it was likely Ctrl-C.
            process::exit(0);
        }
        Ok(_) => Ok(input.trim_right().to_string()),
        Err(error) => Err(format!("Failed to read from std::cin: {}.", error)),
    }
}

/// Reads a line from from `reader` and strips trailing whitespace.  It returns the value entered if
pub fn get_uint<T: BufRead>(reader: &mut T, default: Option<u64>) -> Result<u64, String> {
    let input = get_string(reader)?;
    let error = "Enter positive integer or zero.".to_string();
    if input.is_empty() {
        return default.ok_or(error);
    }
    input.parse::<u64>().map_err(|_| error)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    fn make_cursor(data: &str) -> Cursor<Vec<u8>> {
        Cursor::new(data.chars().map(|c| c as u8).collect())
    }

    #[test]
    fn get_string() {
        let mut cursor = make_cursor("");
        assert_eq!(unwrap!(super::get_string(&mut cursor)), "");
        cursor = make_cursor("AbCd");
        assert_eq!(unwrap!(super::get_string(&mut cursor)), "AbCd");
        assert_eq!(unwrap!(super::get_string(&mut cursor)), "");
    }

    #[test]
    fn get_uint() {
        let mut cursor = make_cursor("0");
        assert_eq!(unwrap!(super::get_uint(&mut cursor, None)), 0);
        cursor = make_cursor("999999");
        assert_eq!(unwrap!(super::get_uint(&mut cursor, None)), 999999);
        cursor = make_cursor("");
        assert_eq!(unwrap!(super::get_uint(&mut cursor, Some(1234))), 1234);
        cursor = make_cursor("999999");
        assert_eq!(unwrap!(super::get_uint(&mut cursor, Some(1234))), 999999);

        cursor = make_cursor("-1");
        assert!(super::get_uint(&mut cursor, None).is_err());
        cursor = make_cursor("gibberish");
        assert!(super::get_uint(&mut cursor, None).is_err());
        cursor = make_cursor("");
        assert!(super::get_uint(&mut cursor, None).is_err());
    }
}
