use toml_edit::{Item, Table, TableLike, Value, value};

/// One pane's share of `ricochet-config.toml`.
pub trait SettingsSection: Sized {
    /// The pane's answers as saved in `table`, the parsed config file.
    fn from_table(table: &dyn TableLike) -> Self;

    /// Writes the pane's answers into `table`, leaving every key the pane does not manage alone.
    fn apply(&self, table: &mut Table);

    /// What the pane still needs before ricochet can start, or `None` once it is complete.
    fn problem(&self) -> Option<String>;
}

/// Reads typed values out of a parsed table, treating a key of the wrong type as absent.
pub trait TableRead {
    fn child(&self, key: &str) -> Option<&dyn TableLike>;
    fn string(&self, key: &str) -> Option<String>;
    fn int(&self, key: &str) -> Option<i64>;
    fn boolean(&self, key: &str) -> Option<bool>;
    fn strings(&self, key: &str) -> Option<Vec<String>>;
}

impl TableRead for dyn TableLike + '_ {
    fn child(&self, key: &str) -> Option<&dyn TableLike> {
        self.get(key)?.as_table_like()
    }

    fn string(&self, key: &str) -> Option<String> {
        self.get(key)?.as_str().map(str::to_owned)
    }

    fn int(&self, key: &str) -> Option<i64> {
        self.get(key)?.as_integer()
    }

    fn boolean(&self, key: &str) -> Option<bool> {
        self.get(key)?.as_bool()
    }

    fn strings(&self, key: &str) -> Option<Vec<String>> {
        let array = self.get(key)?.as_array()?;
        Some(
            array
                .iter()
                .map(|entry| entry.as_str().unwrap_or("").to_owned())
                .collect(),
        )
    }
}

/// Edits a table in place, in the ways every pane needs.
pub trait TableWrite {
    /// The table at `key`, inserted empty when absent: editing it edits this table.
    fn section(&mut self, key: &str) -> &mut Table;

    /// Sets `key` to `value`, or removes it when `value` is `None`.
    fn set<V: Into<Value>>(&mut self, key: &str, value: Option<V>);

    /// Removes the table at `key` once nothing is left in it.
    fn remove_if_empty(&mut self, key: &str);
}

impl TableWrite for Table {
    fn section(&mut self, key: &str) -> &mut Table {
        match self.get_mut(key) {
            Some(item) if item.is_table() => {}
            Some(item) if item.is_inline_table() => {
                if let Ok(table) = std::mem::take(item).into_table() {
                    *item = Item::Table(table);
                }
            }
            _ => {
                self.insert(key, Item::Table(Table::new()));
            }
        }
        match self.get_mut(key).and_then(Item::as_table_mut) {
            Some(table) => table,
            None => unreachable!("`{key}` was just made a table"),
        }
    }

    fn set<V: Into<Value>>(&mut self, key: &str, new: Option<V>) {
        match new {
            Some(new) => {
                self.insert(key, value(new));
            }
            None => {
                self.remove(key);
            }
        }
    }

    fn remove_if_empty(&mut self, key: &str) {
        if self
            .get(key)
            .and_then(Item::as_table_like)
            .is_some_and(|table| table.is_empty())
        {
            self.remove(key);
        }
    }
}

/// What a text field holds, as the form needs it.
pub trait Field {
    fn trimmed(&self) -> &str;
    /// The trimmed text, or `None` when nothing is left.
    fn non_empty(&self) -> Option<String>;
    fn is_http_url(&self) -> bool;
    /// The whole number typed here, or `None` when the field is empty or not a number.
    fn whole_number(&self) -> Option<i64>;
    /// Whether the field is empty or holds a whole number of at least 1.
    fn is_empty_or_positive(&self) -> bool;
}

impl Field for str {
    fn trimmed(&self) -> &str {
        self.trim()
    }

    fn non_empty(&self) -> Option<String> {
        let text = self.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    fn is_http_url(&self) -> bool {
        let text = self.trim();
        text.starts_with("https://") || text.starts_with("http://")
    }

    fn whole_number(&self) -> Option<i64> {
        self.non_empty().and_then(|text| text.parse().ok())
    }

    fn is_empty_or_positive(&self) -> bool {
        self.non_empty().is_none() || self.whole_number().unwrap_or(0) > 0
    }
}

impl Field for String {
    fn trimmed(&self) -> &str {
        self.as_str().trimmed()
    }
    fn non_empty(&self) -> Option<String> {
        self.as_str().non_empty()
    }
    fn is_http_url(&self) -> bool {
        self.as_str().is_http_url()
    }
    fn whole_number(&self) -> Option<i64> {
        self.as_str().whole_number()
    }
    fn is_empty_or_positive(&self) -> bool {
        self.as_str().is_empty_or_positive()
    }
}
