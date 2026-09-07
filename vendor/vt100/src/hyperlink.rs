/// An immutable OSC 8 destination, shared by all cells painted inside the link.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Hyperlink {
    uri: String,
    id: String,
}

impl Hyperlink {
    /// Builds bounded metadata that can safely be serialized as OSC 8.
    /// Empty destinations close links and are represented by `None` instead.
    #[must_use]
    pub fn new(uri: String, id: String) -> Option<Self> {
        if uri.is_empty()
            || uri.len() > 8192
            || id.len() > 1024
            || uri.chars().any(char::is_control)
            || id.chars().any(|c| c.is_control() || matches!(c, ':' | ';'))
        {
            return None;
        }
        Some(Self { uri, id })
    }

    /// Returns the original destination, independently of the displayed label.
    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the identifier joining parts of a link across redraws and rows.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}
