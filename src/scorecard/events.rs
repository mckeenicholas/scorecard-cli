use std::error::Error;
use std::fmt;

/// Error returned when an unrecognized or unsupported WCA event ID is encountered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEventError(pub String);

impl fmt::Display for UnknownEventError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown or unsupported WCA event ID: '{}'", self.0)
    }
}

impl Error for UnknownEventError {}

/// Maps standard WCA event IDs to their user-friendly display names, or returns an error if unrecognized.
pub fn event_name_by_id(id: &str) -> Result<&'static str, UnknownEventError> {
    let event_name = match id {
        "333" => "3x3x3 Cube",
        "222" => "2x2x2 Cube",
        "444" => "4x4x4 Cube",
        "555" => "5x5x5 Cube",
        "666" => "6x6x6 Cube",
        "777" => "7x7x7 Cube",
        "333bf" => "3x3x3 Blindfolded",
        "333oh" => "3x3x3 One-Handed",
        "333fm" => "3x3x3 Fewest Moves",
        "333ft" => "3x3x3 With Feet",
        "minx" => "Megaminx",
        "pyram" => "Pyraminx",
        "clock" => "Rubik's Clock",
        "skewb" => "Skewb",
        "sq1" => "Square-1",
        "444bf" => "4x4x4 Blindfolded",
        "555bf" => "5x5x5 Blindfolded",
        "333mbf" => "3x3x3 Multi-Blind",
        _ => return Err(UnknownEventError(id.to_string())),
    };

    Ok(event_name)
}
