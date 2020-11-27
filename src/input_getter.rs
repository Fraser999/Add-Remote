use std::{io::BufRead, process};

/// Reads a line from from `reader` and strips trailing whitespace.
pub fn get_string<T: BufRead>(reader: &mut T) -> Result<String, String> {
    let mut input = String::new();
    match reader.read_line(&mut input) {
        Ok(character_count) if character_count == 0 => {
            // A signal was sent - just exit the process as it was likely Ctrl-C.
            process::exit(0);
        }
        Ok(_) => Ok(input.trim_end().to_string()),
        Err(error) => Err(format!("Failed to read from std::cin: {}.", error)),
    }
}

/// Reads a line from `reader`, and strips the trailing whitespace.  It returns true if the line is
/// `Y` or `y`; false if the line is `N` or `n`; the unwrapped `default` value if the line is empty,
/// or else an error.
pub fn get_bool<T: BufRead>(reader: &mut T, default: Option<bool>) -> Result<bool, String> {
    let input = get_string(reader)?;
    let error = "Enter 'y' or 'n' only.".to_string();
    match &*input {
        "Y" | "y" => Ok(true),
        "N" | "n" => Ok(false),
        "" => default.ok_or(error),
        _ => Err(error),
    }
}

/// Reads a line from from `reader` and strips trailing whitespace.  It returns the value entered if
pub fn get_uint<T: BufRead>(reader: &mut T, default: Option<u64>) -> Result<u64, String> {
    let input = get_string(reader)?;
    let error = "Enter positive integer or zero.".to_string();
    if input.is_empty() {
        return default.ok_or(error);
    }
    #[allow(clippy::map_err_ignore)]
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
        let mut cursor = make_cursor("AbCd");
        assert_eq!(super::get_string(&mut cursor).unwrap(), "AbCd");
    }

    #[test]
    fn get_bool() {
        // Cases where `get_bool()` returns Ok(true)
        let mut cursor = make_cursor("Y");
        assert!(super::get_bool(&mut cursor, None).unwrap());
        cursor = make_cursor("Y");
        assert!(super::get_bool(&mut cursor, Some(false)).unwrap());
        cursor = make_cursor("y");
        assert!(super::get_bool(&mut cursor, None).unwrap());
        cursor = make_cursor("y");
        assert!(super::get_bool(&mut cursor, Some(false)).unwrap());

        // Cases where `get_bool()` returns Ok(false)
        cursor = make_cursor("N");
        assert!(!super::get_bool(&mut cursor, None).unwrap());
        cursor = make_cursor("N");
        assert!(!super::get_bool(&mut cursor, Some(true)).unwrap());
        cursor = make_cursor("n");
        assert!(!super::get_bool(&mut cursor, None).unwrap());
        cursor = make_cursor("n");
        assert!(!super::get_bool(&mut cursor, Some(true)).unwrap());

        // Cases where `get_bool()` returns Err
        cursor = make_cursor("yy");
        assert!(super::get_bool(&mut cursor, None).is_err());
        cursor = make_cursor("nn");
        assert!(super::get_bool(&mut cursor, Some(true)).is_err());
    }

    #[test]
    fn get_uint() {
        let mut cursor = make_cursor("0");
        assert_eq!(super::get_uint(&mut cursor, None).unwrap(), 0);
        cursor = make_cursor("999999");
        assert_eq!(super::get_uint(&mut cursor, None).unwrap(), 999_999);
        cursor = make_cursor("999999");
        assert_eq!(super::get_uint(&mut cursor, Some(1234)).unwrap(), 999_999);

        cursor = make_cursor("-1");
        assert!(super::get_uint(&mut cursor, None).is_err());
        cursor = make_cursor("gibberish");
        assert!(super::get_uint(&mut cursor, None).is_err());
    }
}
