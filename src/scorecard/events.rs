/// Maps standard WCA event IDs to their user-friendly display names, or returns None if unrecognized.
pub fn event_name_by_id(id: &str) -> Option<&'static str> {
    match id {
        "333" => Some("3x3x3 Cube"),
        "222" => Some("2x2x2 Cube"),
        "444" => Some("4x4x4 Cube"),
        "555" => Some("5x5x5 Cube"),
        "666" => Some("6x6x6 Cube"),
        "777" => Some("7x7x7 Cube"),
        "333bf" => Some("3x3x3 Blindfolded"),
        "333oh" => Some("3x3x3 One-Handed"),
        "333fm" => Some("3x3x3 Fewest Moves"),
        "333ft" => Some("3x3x3 With Feet"),
        "minx" => Some("Megaminx"),
        "pyram" => Some("Pyraminx"),
        "clock" => Some("Rubik's Clock"),
        "skewb" => Some("Skewb"),
        "sq1" => Some("Square-1"),
        "444bf" => Some("4x4x4 Blindfolded"),
        "555bf" => Some("5x5x5 Blindfolded"),
        "333mbf" => Some("3x3x3 Multi-Blind"),
        _ => None,
    }
}
