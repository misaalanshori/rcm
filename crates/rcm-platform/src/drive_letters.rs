/// Returns a list of available (unassigned) Windows drive letters in reverse alphabetical order (Z..=D)
/// Per MT-1 / §7.7
pub fn get_available_drive_letters() -> Vec<char> {
    #[cfg(windows)]
    {
        let mut available = Vec::new();
        for letter in ('D'..='Z').rev() {
            if is_drive_letter_available(letter) {
                available.push(letter);
            }
        }
        available
    }

    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn is_drive_letter_available(letter: char) -> bool {
    let letter = letter.to_ascii_uppercase();
    if !letter.is_ascii_alphabetic() {
        return false;
    }

    #[cfg(windows)]
    {
        let drive_root = format!("{}:\\", letter);
        let path = std::path::Path::new(&drive_root);
        // If metadata succeeds or path exists, the drive letter is in use
        !path.exists()
    }

    #[cfg(not(windows))]
    {
        let _ = letter;
        false
    }
}
