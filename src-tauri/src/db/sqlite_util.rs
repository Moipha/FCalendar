pub fn map_optional<T>(result: Result<T, rusqlite::Error>) -> Result<Option<T>, rusqlite::Error> {
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}
