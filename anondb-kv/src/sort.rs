#[derive(Debug, Clone, Default, PartialEq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

impl ToString for SortDirection {
    fn to_string(&self) -> String {
        match self {
            Self::Asc => "asc".into(),
            Self::Desc => "desc".into(),
        }
    }
}
